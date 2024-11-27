use spl_token::amount_to_ui_amount;

use crate::database::AppDatabase;

pub fn earnings() {
    let app_db = AppDatabase::new();

    let daily_earnings_coal = app_db.get_daily_earnings_coal(7);
    let daily_earnings_ore = app_db.get_daily_earnings_ore(7);

    // print both coal and ore earnings for the same day in the same line
    for (de_coal, de_ore) in daily_earnings_coal.iter().zip(daily_earnings_ore.iter()) {
        println!("Day: {}, Total Earned: {} COAL, {} ORE", de_coal.0, amount_to_ui_amount(de_coal.1, coal_api::consts::TOKEN_DECIMALS), amount_to_ui_amount(de_ore.1, ore_api::consts::TOKEN_DECIMALS));
    }
}
