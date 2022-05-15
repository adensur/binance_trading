use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use error_chain::error_chain;
error_chain! {
    errors {
        EmptyDbError
        /*EmptyDbError() {
            description("Input file json is empty")
            display("Input file json is empty")
        }*/
    }
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
        JsonDecodeError(serde_json::Error);
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
    time: i64,
    #[serde(rename = "isBuyerMaker")]
    is_buyer_maker: bool,
    #[serde(rename = "isBestMatch")]
    is_best_match: bool,
}

struct Db {
    data: Vec<HistoricalTrade>,
}

impl Db {
    fn new(filename: &str) -> Result<Db> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut deserialized: Vec<HistoricalTrade> = serde_json::from_reader(reader)?;
        if deserialized.len() == 0 {
            return Err(ErrorKind::EmptyDbError.into());
        }
        deserialized.sort_by(|a, b| a.trade_id.cmp(&b.trade_id));
        Ok(Db { data: deserialized })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let res = reqwest::get("http://httpbin.org/get").await?;
    println!("Status: {}", res.status());
    println!("Headers:\n{:#?}", res.headers());

    let body = res.text().await?;
    println!("Body:\n{}", body);

    let db = Db::new("historical_trades.json")?;
    println!("Id: {:?}", db.data[0].trade_id);

    Ok(())
}
