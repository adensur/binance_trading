use chrono::NaiveDateTime;
use db;
use error_chain::error_chain;
use std::path::PathBuf;
use structopt::StructOpt;

error_chain! {
    links {
        Utils(db::Error, db::ErrorKind);
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: PathBuf,
    #[structopt(short = "c", long = "count")]
    count: i64,
    #[structopt(short = "s", long = "symbol", default_value = "ETHBTC")]
    symbol: String,
}

async fn run() -> Result<()> {
    let opt = Opt::from_args();
    let mut db = db::Db::new(&opt.input)?;
    println!(
        "Id: {}, records count {}, min_ts: {}",
        db.get_min_trade_id(),
        db.get_data_len(),
        NaiveDateTime::from_timestamp(db.get_min_time_milliseconds() / 1000, 0)
    );

    for i in 0..opt.count {
        db.load_more_data(&opt.symbol).await?;
        println!(
            "Id: {}, records count {}, min_ts: {}",
            db.get_min_trade_id(),
            db.get_data_len(),
            NaiveDateTime::from_timestamp(db.get_min_time_milliseconds() / 1000, 0)
        );
        if i % 100 == 0 {
            println!("Processing {} out out {}", i, opt.count);
        }
    }

    db.save(&opt.input)?;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(ref e) = run().await {
        println!("error: {}", e);

        for e in e.iter().skip(1) {
            println!("caused by: {}", e);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            println!("backtrace: {:?}", backtrace);
        }

        ::std::process::exit(1);
    }
}
