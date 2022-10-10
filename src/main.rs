use anyhow::Result;
use log::{error, info};
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

use tokio_cron_scheduler::{Job, JobScheduler};

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

#[derive(Clone)]
struct SakiHomeBotConfig {
    tenid: String,
    set_hour: u8,
    admin_chat_id: i64,
    warn_dianfei: f32,
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

    let sched = JobScheduler::new().await.unwrap();

    dotenv::dotenv().ok();
    let bot = Bot::from_env().auto_send();

    let config = SakiHomeBotConfig {
        tenid: std::env::var("TENID").expect("Can not get TENID in var"),
        set_hour: std::env::var("HOUR")
            .unwrap_or("7".to_string())
            .parse()
            .unwrap(),
        admin_chat_id: std::env::var("CHAT_ID")
            .expect("Can not get CHAT_ID in var")
            .parse()
            .unwrap(),
        warn_dianfei: std::env::var("WARN_DIANFEI")
            .unwrap_or("30".to_string())
            .parse()
            .unwrap(),
    };

    let bot_clone = bot.clone();
    let config_clone = config.clone();

    sched
        .add(
            Job::new_async(
                format!("0 {} * * *", config.set_hour).as_str(),
                move |_, _| {
                    let bot_clone = bot_clone.clone();
                    let config_clone = config_clone.clone();
                    Box::pin(async move {
                        if let Err(e) = time_to_get_electricity_inner(config_clone, bot_clone).await
                        {
                            error!("{}", e);
                        }
                    })
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();

    tokio::try_join!(
        async {
            teloxide::commands_repl(bot.clone(), answer, Command::ty()).await;

            Ok(())
        },
        async {
            sched.start().await?;

            Ok(())
        },
        time_to_pay_electricity(config.clone(), bot.clone()),
    )
    .unwrap();
}

async fn answer(
    bot: AutoSend<Bot>,
    message: Message,
    command: Command,
    config: SakiHomeBotConfig,
) -> Result<()> {
    match command {
        Command::DianFei => {
            bot_get_electricity(config, bot, message.chat.id).await?;
        }
    }

    Ok(())
}

async fn bot_get_electricity<C: Into<Recipient> + Copy>(
    config: SakiHomeBotConfig,
    bot: AutoSend<Bot>,
    chat_id: C,
) -> Result<()> {
    let electricitys = get_electricity(&config.tenid).await?;

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

async fn get_electricity(tenid: &str) -> Result<Vec<TangChaoElectricityResult>> {
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

async fn time_to_get_electricity_inner(
    config: SakiHomeBotConfig,
    bot: AutoSend<Bot>,
) -> Result<()> {
    let chat_id = config.admin_chat_id;
    bot_get_electricity(config, bot, ChatId(chat_id)).await?;

    Ok(())
}

async fn time_to_pay_electricity(config: SakiHomeBotConfig, bot: AutoSend<Bot>) -> Result<()> {
    let es = get_electricity(&config.tenid).await?;

    let warn = config.warn_dianfei;

    for i in es {
        if i.smart_balance < warn {
            bot.send_message(
                ChatId(config.admin_chat_id),
                format!(
                    "{} 的电费小于 {} 啦！目前余额为： {}，快充值！！！",
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
