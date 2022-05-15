use db;
use rand::Rng;

enum TradeAction {
    // Pass, // just use Buy(0)
    BuyQuote { base_quantity: f64 }, // exchainge base_quantity of base symbol for last_price * quote_quantity
                                     // Sell { base_quantity: f64 }, // just use Buy(-qty)
}

//const FEE: f64 = 0.001;
//const FEE: f64 = 0.0001;
const FEE: f64 = 0.00001;

trait Strategy {
    fn new(base_balance: f64, quote_balance: f64) -> Box<dyn Strategy>
    where
        Self: Sized;
    fn react_to_data(
        &mut self,
        new_base_balance: f64, // new balances after previous action (if any)
        new_quote_balance: f64,
        new_data: &db::HistoricalTrade,
    ) -> TradeAction;
}

struct DummyStrategy {
    _base_balance: f64,
    _quote_balance: f64,
}

impl Strategy for DummyStrategy {
    fn new(base_balance: f64, quote_balance: f64) -> Box<dyn Strategy> {
        let strategy = DummyStrategy {
            _base_balance: base_balance,
            _quote_balance: quote_balance,
        };
        Box::new(strategy)
    }
    fn react_to_data(
        &mut self,
        _new_base_balance: f64,
        _new_quote_balance: f64,
        _new_data: &db::HistoricalTrade,
    ) -> TradeAction {
        TradeAction::BuyQuote { base_quantity: 0.0 }
    }
}

struct RandomStrategy {
    base_balance: f64,
    quote_balance: f64,
    last_buying_price: Option<f64>,
    already_sold: bool,
}

impl Strategy for RandomStrategy {
    fn new(base_balance: f64, quote_balance: f64) -> Box<dyn Strategy> {
        let strategy = RandomStrategy {
            base_balance: base_balance,
            quote_balance: quote_balance,
            last_buying_price: None,
            already_sold: false,
        };
        Box::new(strategy)
    }
    fn react_to_data(
        &mut self,
        new_base_balance: f64,
        new_quote_balance: f64,
        new_data: &db::HistoricalTrade,
    ) -> TradeAction {
        self.base_balance = new_base_balance;
        self.quote_balance = new_quote_balance;
        if self.already_sold {
            return TradeAction::BuyQuote { base_quantity: 0.0 };
        }
        /*
            buy for all, then wait until price increased and sell all
        */
        match self.last_buying_price {
            None => {
                self.last_buying_price = Some(new_data.get_price() * (1.0 + FEE));
                TradeAction::BuyQuote { base_quantity: 1.0 }
            }
            Some(last_buying_price) => {
                let new_price = new_data.get_price();
                if new_price * (1.0 + FEE) < last_buying_price * (1.0 - FEE) {
                    self.already_sold = true;
                    return TradeAction::BuyQuote {
                        base_quantity: -self.quote_balance / new_price * 0.999999 * (1.0 - FEE),
                    };
                }
                TradeAction::BuyQuote { base_quantity: 0.0 }
            }
        }
    }
}

struct Executor {
    db: db::Db,
}

impl Executor {
    fn new(filename: &str) -> Executor {
        let db = db::Db::new(&filename).unwrap();
        Executor { db: db }
    }
    fn simulate_strategy<T: Strategy>(&self, verbose: bool) -> (f64, f64) {
        let base_name = "ETH";
        let quote_name = "BTC";
        let mut rng = rand::thread_rng();
        let start_id: usize = rng.gen_range(0..self.db.get_data_len());
        let finish_id: usize = rng.gen_range(start_id..self.db.get_data_len());
        let mut base_balance: f64 = 1.0;
        let mut quote_balance: f64 = 0.0;
        let mut strategy = T::new(base_balance, quote_balance);
        if verbose {
            println!("Generated id: {}-{}", start_id, finish_id);
        }
        for i in start_id..finish_id {
            let new_data = self.db.get_data(i);
            let action = strategy.react_to_data(base_balance, quote_balance, new_data);
            match action {
                TradeAction::BuyQuote { base_quantity } => {
                    let last_price = new_data.get_price();
                    base_balance -= base_quantity;
                    let quote_diff: f64;
                    if base_quantity >= 0.0 {
                        // quote diff is positive - we get less then we paid for
                        quote_diff = base_quantity * last_price * (1.0 - FEE);
                    } else {
                        // quote diff is negative - we pay out more than we get back
                        quote_diff = base_quantity * last_price * (1.0 + FEE);
                    }
                    quote_balance += quote_diff;
                    if verbose {
                        if base_quantity != 0.0 {
                            println!("Current price: {last_price}, Buying {quote_diff} of {quote_name} for {base_quantity} {base_name}; New {base_name} balance: {base_balance}; new {quote_name} balance: {quote_balance}");
                        } else {
                            println!("Current price: {last_price}");
                        }
                    }
                    if base_balance < 0.0 {
                        panic!("base_balance < 0! {}", base_balance)
                    }
                    if quote_balance < 0.0 {
                        panic!("quote_balance < 0! {}", quote_balance)
                    }
                }
            }
        }
        if verbose {
            println!("Final bot base balance: {base_balance}; quote_balance: {quote_balance}");
        }
        (base_balance, quote_balance)
    }
}

fn main() {
    let executor = Executor::new("../hist_getter/historical_trades.json");
    println!("Db data len: {}", executor.db.get_data_len());
    let mut success_count = 0;
    let mut total_count = 0;
    for _ in 0..10000 {
        let (base_balance, _quote_balance) = executor.simulate_strategy::<RandomStrategy>(false);
        total_count += 1;
        if base_balance > 1.0 {
            success_count += 1;
        }
    }
    println!("success count: {success_count}, total_count: {total_count}")
}
