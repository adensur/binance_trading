use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use error_chain::error_chain;
error_chain! {
    errors {
        EmptyDbError
        ApiKeyNotFoundError {
            description("No api key found in env variable. Please set it to BINANCE_API_KEY")
            display("No api key found in env variable. Please set it to BINANCE_API_KEY")
        }
        IntersectingTradeSlicesError(old_id: i64, new_id: i64) {
            description("Loaded trade data intersects with old trade data")
            display("Loaded trade data intersects with old trade data; old_id: '{}', new_id: '{}'", old_id, new_id)
        }
    }
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
        JsonDecodeError(serde_json::Error);
        MissingApiKeyInEnv(std::env::VarError);
    }
}

/*
    {
        "id": 340327051,
        "price": "0.06901500",
        "qty": "0.00160000",
        "quoteQty": "0.00011042",
        "time": 1652614347356,
        "isBuyerMaker": false,
        "isBestMatch": true
    },
*/
#[derive(Serialize, Deserialize)]
struct HistoricalTrade {
    #[serde(rename = "id")]
    trade_id: i64,
    #[serde(rename = "price")]
    price: String,
    #[serde(rename = "qty")]
    quantity: String,
    #[serde(rename = "quoteQty")]
    quote_quantity: String,
    #[serde(rename = "time")]
    time_milliseconds: i64,
    #[serde(rename = "isBuyerMaker")]
    is_buyer_maker: bool,
    #[serde(rename = "isBestMatch")]
    is_best_match: bool,
}

pub struct Db {
    data: Vec<HistoricalTrade>,
    min_trade_id: i64,
    min_time_milliseconds: i64,
}

impl Db {
    pub fn get_min_trade_id(&self) -> i64 {
        self.data[0].trade_id
    }
    pub fn get_min_time_milliseconds(&self) -> i64 {
        self.data[0].time_milliseconds
    }
    pub fn get_data_len(&self) -> usize {
        self.data.len()
    }
    pub fn new<P: AsRef<Path>>(filename: &P) -> Result<Db> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut deserialized: Vec<HistoricalTrade> = serde_json::from_reader(reader)?;
        if deserialized.len() == 0 {
            return Err(ErrorKind::EmptyDbError.into());
        }
        deserialized.sort_by(|a, b| a.trade_id.cmp(&b.trade_id));
        Ok(Db {
            min_trade_id: deserialized[0].trade_id,
            min_time_milliseconds: deserialized[0].time_milliseconds,
            data: deserialized,
        })
    }
    pub async fn load_more_data(&mut self) -> Result<()> {
        let limit = 1000;
        let from_id = self.min_trade_id - limit;
        let query = format!("https://api.binance.com/api/v3/historicalTrades?symbol=ETHBTC&limit={limit}&fromId={from_id}");
        let client = reqwest::Client::new();
        let api_key = env::var("BINANCE_API_KEY").chain_err(|| ErrorKind::ApiKeyNotFoundError)?;
        let res = client
            .get(query)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
        let mut new_data = res.json::<Vec<HistoricalTrade>>().await?;
        if new_data.len() == 0 {
            return Err(ErrorKind::EmptyDbError.into());
        }
        if new_data[0].trade_id >= self.min_trade_id {
            return Err(ErrorKind::IntersectingTradeSlicesError(
                self.min_trade_id,
                new_data[0].trade_id,
            )
            .into());
        }
        new_data.sort_by(|a, b| a.trade_id.cmp(&b.trade_id));
        new_data.extend(self.data.drain(..));
        self.data = new_data;
        self.min_trade_id = self.data[0].trade_id;
        self.min_time_milliseconds = self.data[0].time_milliseconds;
        Ok(())
    }
    pub fn save<P: AsRef<Path>>(&self, filename: &P) -> Result<()> {
        let file = File::create(filename)?;
        serde_json::to_writer(BufWriter::new(file), &self.data)?;
        Ok(())
    }
}
