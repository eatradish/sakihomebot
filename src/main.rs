use std::error::Error;

use anyhow::Result;
use log::info;
use reqwest::{header::HeaderMap, Client};
use serde::Deserialize;
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    dptree,
    payloads::SendMessageSetters,
    prelude::AutoSend,
    prelude::Dispatcher,
    requests::{Requester, RequesterExt},
    types::{ChatId, Message, ParseMode, Recipient, Update},
    utils::command::BotCommands,
    Bot,
};
use time::{macros::offset, OffsetDateTime};

#[derive(Deserialize, Debug)]
struct TangChaoElectricity {
    result: Vec<TangChaoElectricityResult>,
}

#[derive(Deserialize, Debug)]
struct TangChaoElectricityResult {
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Room")]
    room: String,
    #[serde(rename = "SmartBalance")]
    smart_balance: f32,
}

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "Get Electricity.")]
    DianFei,
}

type TeloxideHandleResult = Result<(), Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting saki home bot...");

    dotenv::dotenv().ok();
    let bot = Bot::from_env().auto_send();

    Dispatcher::builder(
        bot,
        Update::filter_message()
            .branch(dptree::entry().filter_command::<Command>().endpoint(answer))
            .branch(dptree::entry().endpoint(time_to_get_electricity))
            .branch(dptree::entry().endpoint(time_to_pay_electricity)),
    )
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
}

async fn answer(bot: AutoSend<Bot>, message: Message, command: Command) -> TeloxideHandleResult {
    match command {
        Command::DianFei => {
            bot_get_electricity(bot, message.chat.id).await?;
        }
    }

    Ok(())
}

async fn bot_get_electricity<C: Into<Recipient> + Copy>(
    bot: AutoSend<Bot>,
    chat_id: C,
) -> TeloxideHandleResult {
    let electricitys = get_electricity().await?;

    for i in electricitys {
        bot.send_message(
            chat_id,
            format!("<b>{} {}</b>\n{}", i.address, i.room, i.smart_balance),
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }

    Ok(())
}

async fn get_electricity() -> Result<Vec<TangChaoElectricityResult>> {
    let tenid = std::env::var("TENID").expect("Can not get tenid");
    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 6_1_3 like Mac OS X) AppleWebKit/536.26 (KHTML, like Gecko) Mobile/10B329 MicroMessenger/5.0.1".parse()?);
    headers.insert("Accept", "application/json, text/plain, */*".parse()?);
    headers.insert("Origin", "http://www.4006269069.net".parse()?);
    headers.insert("Referer", "http://www.4006269069.net".parse()?);
    headers.insert("Accept-Encoding", "gzip, deflate".parse()?);
    headers.insert("Accept-Language", "en-US,en;q=0.5".parse()?);
    headers.insert("Head-User-Id", "".parse()?);

    let url = format!("http://api.4006269069.net/wechat/tenant/rentadviser/RdWxPact/getRdInteDeviceHouConList?TenId={}", tenid);

    let res = client
        .get(url)
        .headers(headers)
        .send()
        .await?
        .error_for_status()?
        .json::<TangChaoElectricity>()
        .await?
        .result;

    Ok(res)
}

async fn time_to_get_electricity(bot: AutoSend<Bot>) -> TeloxideHandleResult {
    let time = OffsetDateTime::now_utc().to_offset(offset!(+8));
    let hour = time.hour();

    let set_hour = if let Ok(h) = std::env::var("HOUR") {
        h.parse::<u8>()?
    } else {
        return Ok(());
    };

    let chat_id = if let Ok(chat_id) = std::env::var("CHAT_ID") {
        chat_id.parse::<i64>()?
    } else {
        return Ok(());
    };

    if hour == set_hour {
        bot_get_electricity(bot, ChatId(chat_id)).await?;
    }

    Ok(())
}

async fn time_to_pay_electricity(bot: AutoSend<Bot>) -> TeloxideHandleResult {
    let es = get_electricity().await?;

    let chat_id = if let Ok(chat_id) = std::env::var("CHAT_ID") {
        chat_id.parse::<i64>()?
    } else {
        return Ok(());
    };

    let warn = std::env::var("WARN_DIANFEI")
        .unwrap_or_else(|_| "30".to_string())
        .parse::<f32>()?;

    for i in es {
        if i.smart_balance < warn {
            bot.send_message(
                ChatId(chat_id),
                format!(
                    "{} 的电费小于 {} 啦！，目前余额为： {}，快充值！！！",
                    format_args!("{} {}", i.address, i.room),
                    warn,
                    i.smart_balance
                ),
            )
            .await?;
        }
    }

    Ok(())
}
