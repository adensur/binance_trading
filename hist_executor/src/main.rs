use db;
use rand::Rng;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Copy, Clone)]
struct Balance {
    base_balance: f64,
    quote_balance: f64,
}

impl Balance {
    fn buy(&mut self, base_quantity: f64, fee: f64, price: f64) {
        if base_quantity < 0.0 {
            panic!("CHEETAH!");
        }
        self.base_balance -= base_quantity;
        let quote_diff: f64;
        quote_diff = base_quantity * price * (1.0 - fee);
        self.quote_balance += quote_diff;
        if self.base_balance < 0.0 {
            panic!("base_balance < 0! {}", self.base_balance)
        }
        if self.quote_balance < 0.0 {
            panic!("quote_balance < 0! {}", self.quote_balance)
        }
    }
    fn sell(&mut self, quote_quantity: f64, fee: f64, price: f64) {
        if quote_quantity < 0.0 {
            panic!("CHEETAH!");
        }
        let base_diff = quote_quantity * 1.0 / price * (1.0 - fee);
        self.quote_balance -= quote_quantity;
        self.base_balance += base_diff;
        if self.base_balance < 0.0 {
            panic!("base_balance < 0! {}", self.base_balance)
        }
        if self.quote_balance < 0.0 {
            panic!("quote_balance < 0! {}", self.quote_balance)
        }
    }
}

enum TradeAction {
    Pass,
    BuyQuote { base_quantity: f64 }, // exchange base_quantity of base symbol for last_price * quote_quantity * (1 - fee)
    SellQuote { quote_quantity: f64 }, // exchange quote_quantity of quote symbol for 1/last_price * quote_quantity * (1 - fee)
}

trait Strategy {
    fn new(balance: Balance, fee: f64) -> Box<dyn Strategy>
    where
        Self: Sized;
    fn react_to_data(
        &mut self,
        new_balance: Balance, // new balances after previous action (if any)
        new_data: &db::HistoricalTrade,
    ) -> TradeAction;
    fn consume_data(&mut self, new_data: &db::HistoricalTrade); // view historical data, but can't react to it
}

struct DummyStrategy {
    _balance: Balance,
}

impl Strategy for DummyStrategy {
    fn new(balance: Balance, _fee: f64) -> Box<dyn Strategy> {
        let strategy = DummyStrategy { _balance: balance };
        Box::new(strategy)
    }
    fn react_to_data(
        &mut self,
        _new_balance: Balance,
        _new_data: &db::HistoricalTrade,
    ) -> TradeAction {
        TradeAction::BuyQuote { base_quantity: 0.0 }
    }
    fn consume_data(&mut self, _new_data: &db::HistoricalTrade) {
        // pass
    }
}

struct RandomStrategy {
    balance: Balance,
    last_buying_price: Option<f64>,
    already_sold: bool,
    fee: f64,
}

impl Strategy for RandomStrategy {
    fn new(balance: Balance, fee: f64) -> Box<dyn Strategy> {
        let strategy = RandomStrategy {
            balance: balance,
            fee: fee,
            last_buying_price: None,
            already_sold: false,
        };
        Box::new(strategy)
    }
    fn consume_data(&mut self, _new_data: &db::HistoricalTrade) {
        // pass
    }
    fn react_to_data(
        &mut self,
        new_balance: Balance,
        new_data: &db::HistoricalTrade,
    ) -> TradeAction {
        self.balance = new_balance;
        if self.already_sold {
            return TradeAction::BuyQuote { base_quantity: 0.0 };
        }
        /*
            buy for all, then wait until price increased and sell all
        */
        match self.last_buying_price {
            None => {
                self.last_buying_price = Some(new_data.get_price() * (1.0 + self.fee));
                TradeAction::BuyQuote {
                    base_quantity: self.balance.base_balance,
                }
            }
            Some(last_buying_price) => {
                let new_price = new_data.get_price();
                if new_price * (1.0 + self.fee) < last_buying_price * (1.0 - self.fee) {
                    self.already_sold = true;
                    return TradeAction::SellQuote {
                        quote_quantity: self.balance.quote_balance,
                    };
                }
                TradeAction::Pass
            }
        }
    }
}

struct StaticAvgStrategy {
    balance: Balance,
    last_buying_price: Option<f64>,
    already_sold: bool,
    fee: f64,
}

struct Executor {
    db: db::Db,
}

impl Executor {
    fn new<F: AsRef<Path>>(filename: F) -> Executor {
        let db = db::Db::new(&filename).unwrap();
        Executor { db: db }
    }
    fn simulate_strategy<T: Strategy>(&self, fee: f64, verbose: bool) -> Balance {
        let mut rng = rand::thread_rng();
        let start_id: usize = rng.gen_range(0..self.db.get_data_len());
        let finish_id: usize = rng.gen_range(start_id..self.db.get_data_len());
        let mut balance = Balance {
            base_balance: 1.0,
            quote_balance: 0.0,
        };
        let mut strategy = T::new(balance, fee);
        if verbose {
            println!("Generated id: {}-{}", start_id, finish_id);
        }
        let mut last_price = self.db.get_data(start_id).get_price();
        for i in start_id..finish_id {
            let new_data = self.db.get_data(i);
            let action = strategy.react_to_data(balance, new_data);
            last_price = new_data.get_price();
            match action {
                TradeAction::Pass => (),
                TradeAction::SellQuote { quote_quantity } => {
                    if quote_quantity < 0.0 {
                        panic!("CHEETAH!");
                    }
                    balance.sell(quote_quantity, fee, last_price);
                    if verbose {
                        println!("Sell! Current price: {last_price}, base_balance: {}, quote_balance: {}", balance.base_balance, balance.quote_balance);
                    }
                }
                TradeAction::BuyQuote { base_quantity } => {
                    balance.buy(base_quantity, fee, last_price);
                    if verbose {
                        println!(
                            "Buy! Current price: {last_price}, base_balance: {}, quote_balance: {}",
                            balance.base_balance, balance.quote_balance
                        );
                    }
                }
            }
        }
        if verbose {
            println!(
                "Final bot base balance: {}; quote_balance: {}",
                balance.base_balance, balance.quote_balance
            );
        }
        balance.sell(balance.quote_balance, fee, last_price);
        balance
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: PathBuf,
    #[structopt(short = "c", long = "count")]
    count: i64,
    #[structopt(short = "f", long = "fee", default_value = "0.001")]
    fee: f64,
}

fn main() {
    let opt = Opt::from_args();
    let executor = Executor::new(&opt.input);
    println!("Db data len: {}", executor.db.get_data_len());
    let mut success_count = 0;
    let mut draw_count = 0;
    let mut total_count = 0;
    for _ in 0..opt.count {
        let balance = executor.simulate_strategy::<RandomStrategy>(opt.fee, false);
        total_count += 1;
        if balance.base_balance > 1.0 {
            success_count += 1;
        } else if balance.base_balance == 1.0 {
            draw_count += 1;
        }
    }
    println!("success count: {success_count}, draw_count: {draw_count}, total_count: {total_count}")
}
