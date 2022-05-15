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
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let db = db::Db::new(&opt.input)?;
    let mut trades = db.get_all_data_cloned();
    for trade in &mut trades {
        trade.price = format!("{}", 1.0 / trade.get_price());
        std::mem::swap(&mut trade.quantity, &mut trade.quote_quantity);
    }
    let new_db = db::Db::from(trades)?;
    new_db.save(&opt.output)?;
    db.save(&"tmp.json")?;
    Ok(())
}
