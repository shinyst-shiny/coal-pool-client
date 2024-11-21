use serde::Deserialize;
use solana_sdk::{signature::Keypair, signer::Signer};

#[derive(Deserialize)]
pub struct MinerRewards {
    pub coal: f64,
    pub ore: f64,
}
#[derive(Deserialize)]
pub struct MinerBalance {
    pub coal: f64,
    pub ore: f64,
}
pub async fn balance(key: &Keypair, url: String, unsecure: bool) {
    let base_url = url;
    let client = reqwest::Client::new();

    let url_prefix = if unsecure {
        "http".to_string()
    } else {
        "https".to_string()
    };

    println!("Wallet: {}", key.pubkey().to_string());

    // Fetch Wallet (Stakable) Balance
    let balance_response = client
        .get(format!(
            "{}://{}/miner/balance?pubkey={}",
            url_prefix,
            base_url,
            key.pubkey().to_string()
        ))
        .send()
        .await
        .unwrap();

    let mut balance: MinerBalance = MinerBalance { coal: 0.0, ore: 0.0 };
    match balance_response.json::<MinerBalance>().await {
        Ok(balance_resp) => {
            balance = balance_resp;
        }
        Err(_) => {}
    }

    // Fetch Unclaimed Rewards
    let rewards_response = client
        .get(format!(
            "{}://{}/miner/rewards?pubkey={}",
            url_prefix,
            base_url,
            key.pubkey().to_string()
        ))
        .send()
        .await
        .unwrap();

    let mut rewards: MinerRewards = MinerRewards { coal: 0.0, ore: 0.0 };
    match rewards_response.json::<MinerRewards>().await {
        Ok(rewards_resp) => {
            rewards = rewards_resp;
        }
        Err(_) => {}
    }

    println!("  Unclaimed Rewards: {:.11} COAL", rewards.coal);
    println!("  Wallet:            {:.11} COAL", balance.coal);
    println!("  Unclaimed Rewards: {:.11} ORE", rewards.ore);
    println!("  Wallet:            {:.11} ORE", balance.ore);
}

pub async fn get_balance(key: &Keypair, url: String, unsecure: bool) -> f64 {
    let client = reqwest::Client::new();
    let url_prefix = if unsecure { "http" } else { "https" };

    let balance_response = client
        .get(format!(
            "{}://{}/miner/balance?pubkey={}",
            url_prefix,
            url,
            key.pubkey().to_string()
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    balance_response.parse::<f64>().unwrap_or(0.0)
}

pub async fn get_token_balance(key: &Keypair, url: String, unsecure: bool, mint: String) -> f64 {
    let client = reqwest::Client::new();
    let url_prefix = if unsecure { "http" } else { "https" };

    let balance_response = client
        .get(format!(
            "{}://{}/v2/miner/balance?pubkey={}&mint={}",
            url_prefix,
            url,
            key.pubkey().to_string(),
            mint
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    balance_response.parse::<f64>().unwrap_or(0.0)
}