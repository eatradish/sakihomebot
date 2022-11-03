use std::time::Duration;

use anyhow::Result;
use log::{info, error};
use reqwest::{header::HeaderMap, Client};
use serde::Deserialize;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    requests::{Requester, RequesterExt},
    types::{ChatId, Message, ParseMode, Recipient},
    utils::command::BotCommands,
    Bot,
};

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting saki home bot...");

    dotenv::dotenv().ok();
    let bot = Bot::from_env().auto_send();

    let set_hour = get_env("HOUR")
        .expect("Can not get HOUR")
        .parse::<u64>()
        .expect("Can not convert time to u64");

    info!("Set every {} hour send electricity bill", set_hour);

    let chat_id = get_env("CHAT_ID")
        .expect("Can not get chat id")
        .parse::<i64>()
        .expect("Can not convert to i64");

    info!("You are {}!", chat_id);

    let warn_dianfei = get_env("WARN_DIANFEI")
        .unwrap_or("30".to_string())
        .parse::<f32>()
        .unwrap();

    info!("Set electricity bill < {} to send warn", warn_dianfei);

    while let Err(e) = tokio::select! {
        v = async {
            teloxide::commands_repl(bot.clone(), answer, Command::ty()).await;

            Ok(())
        } => v,
        v = time_to_pay_electricity(chat_id, bot.clone(), warn_dianfei) => v,
        v = time_to_get_electricity(chat_id, bot.clone(), set_hour) => v,
    } {
        error!("{}", e);
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    // tokio::select!(
    //     v = {
    //         teloxide::commands_repl(bot.clone(), answer, Command::ty()).await;

    //         Ok(())
    //     } => v.unwrap(),
    //     time_to_pay_electricity(chat_id, bot.clone(), warn_dianfei),
    //     time_to_get_electricity(chat_id, bot.clone(), set_hour),
    // )
    // .unwrap();
}

fn get_env(var: &str) -> Result<String> {
    let var = std::env::var(var)?;

    Ok(var)
}

async fn answer(bot: AutoSend<Bot>, message: Message, command: Command) -> Result<()> {
    match command {
        Command::DianFei => {
            info!("/dianfei by user: {}", message.chat.id);
            bot_get_electricity(bot, message.chat.id).await?;
        }
    }

    Ok(())
}

async fn bot_get_electricity<C: Into<Recipient> + Copy>(
    bot: AutoSend<Bot>,
    chat_id: C,
) -> Result<()> {
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
    let tenid = get_env("TENID")?;
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

    info!("result: {:?}", &res);

    Ok(res)
}

async fn time_to_get_electricity(chat_id: i64, bot: AutoSend<Bot>, sleep_hour: u64) -> Result<()> {
    loop {
        bot_get_electricity(bot.clone(), ChatId(chat_id)).await?;
        tokio::time::sleep(Duration::from_secs(sleep_hour * 60 * 60)).await;
    }
}

async fn time_to_pay_electricity(chat_id: i64, bot: AutoSend<Bot>, warn: f32) -> Result<()> {
    loop {
        let es = get_electricity().await?;

        for i in es {
            if i.smart_balance < warn {
                bot.send_message(
                    ChatId(chat_id),
                    format!(
                        "{} 的电费小于 {} 啦！目前余额为：{}，快充值！！！",
                        format_args!("{} {}", i.address, i.room),
                        warn,
                        i.smart_balance
                    ),
                )
                .await?;
            }
        }

        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}
