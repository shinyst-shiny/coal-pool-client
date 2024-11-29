use crate::balance::{MinerBalance, MinerRewards};
use base64::{prelude::BASE64_STANDARD, Engine};
use clap::Parser;
use colored::*;
use inquire::{InquireError, Text};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use spl_token::amount_to_ui_amount;
use std::{str::FromStr, time::Duration};

#[derive(Debug, Parser)]
pub struct ClaimArgs {
    #[arg(
        long,
        short('r'),
        value_name = "RECEIVER_PUBKEY",
        help = "Wallet Public Key to receive the claimed Coal to."
    )]
    pub receiver_pubkey: Option<String>,
    #[arg(
        long,
        value_name = "AMOUNT",
        help = "Amount of coal to claim. (Minimum of 1 COAL)"
    )]
    pub amount: Option<f64>,
    #[arg(long, short, action, help = "Auto approve confirmations.")]
    pub y: bool,
}

pub async fn claim(args: ClaimArgs, key: Keypair, url: String, unsecure: bool) {
    let client = reqwest::Client::new();
    let url_prefix = if unsecure {
        "http".to_string()
    } else {
        "https".to_string()
    };

    let receiver_pubkey = match args.receiver_pubkey {
        Some(rpk) => match Pubkey::from_str(&rpk) {
            Ok(pk) => pk,
            Err(_) => {
                println!("Failed to parse provided receiver pubkey.\nDouble check the provided public key is valid and try again.");
                return;
            }
        },
        None => key.pubkey(),
    };

    let balance_response = client
        .get(format!(
            "{}://{}/miner/balance?pubkey={}",
            url_prefix,
            url,
            receiver_pubkey.to_string()
        ))
        .send()
        .await
        .unwrap();

    let mut balance: MinerBalance = MinerBalance {
        coal: 0.0,
        ore: 0.0,
        chromium: 0.0,
    };
    match balance_response.json::<MinerBalance>().await {
        Ok(balance_resp) => {
            balance = balance_resp;
        }
        Err(_) => {}
    }

    println!("\nNote: If you don't have a COAL token account a 4 COAL fee will be deducted from your claim amount to cover the cost of Token Account Creation.\
    \nIf you don't have or an ORE token account a 0.02 ORE fee will be deducted from your claim amount to cover the cost of Token Account Creation.\
    \nIf you don't have or a CHROMIUM token account a 4 COAL fee OR a 0.02 ORE fee will be deducted from your claim amount, depending on the token that reaches the minimum claim limit to cover the cost of Token Account Creation.\
    \nThis is a one time fee used to create the COAL and/or ORE and/or CHROMIUM Token Account and will be applied only if you don't have it already.\n");

    let rewards_response = client
        .get(format!(
            "{}://{}/miner/rewards?pubkey={}",
            url_prefix,
            url,
            key.pubkey().to_string()
        ))
        .send()
        .await
        .unwrap();

    let mut rewards: MinerRewards = MinerRewards {
        coal: 0.0,
        ore: 0.0,
        chromium: 0.0,
    };
    match rewards_response.json::<MinerRewards>().await {
        Ok(rewards_resp) => {
            rewards = rewards_resp;
        }
        Err(_) => {}
    }

    println!("  Miner Unclaimed Rewards:       {:.11} COAL", rewards.coal);
    println!("  Miner Unclaimed Rewards:       {:.11} ORE", rewards.ore);
    println!(
        "  Miner Unclaimed Rewards:       {:.11} CHROMIUM",
        rewards.chromium
    );
    println!("  Receiving Wallet COAL Balance: {:.11} COAL", balance.coal);
    println!("  Receiving Wallet ORE Balance:  {:.11} ORE", balance.ore);
    println!(
        "  Receiving Wallet ORE Balance:  {:.11} CHROMIUM",
        balance.chromium
    );

    let minimum_claim_amount_coal = 1.0;
    let minimum_claim_amount_ore = 0.05;
    if rewards.coal < minimum_claim_amount_coal && rewards.ore < minimum_claim_amount_ore {
        println!();
        println!("  You have not reached the required claim limit of at least 1 COAL OR 0.05 ORE.");
        println!("  Keep mining to accumulate more rewards before you can withdraw.");
        return;
    }

    // Convert balance to grains
    let balance_grains_coal =
        (rewards.coal * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64)) as u64;
    let balance_grains_ore =
        (rewards.ore * 10f64.powf(ore_api::consts::TOKEN_DECIMALS as f64)) as u64;
    let balance_grains_chromium =
        (rewards.chromium * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64)) as u64;

    // If balance is zero, inform the user and return to keypair selection
    if balance_grains_coal == 0 && balance_grains_ore == 0 && balance_grains_chromium == 0 {
        println!("\n  There is no balance to claim.");
        return;
    }

    /*println!(
        "  Adjusting claim amount to the maximum available: {} COAL.",
        amount_to_ui_amount(balance_grains_coal, coal_api::consts::TOKEN_DECIMALS)
    );
    println!(
        "  Adjusting claim amount to the maximum available: {} ORE.",
        amount_to_ui_amount(balance_grains_ore, ore_api::consts::TOKEN_DECIMALS)
    );
    println!(
        "  Adjusting claim amount to the maximum available: {} CHROMIUM.",
        amount_to_ui_amount(balance_grains_chromium, ore_api::consts::TOKEN_DECIMALS)
    );*/

    // RED TEXT
    if !args.y {
        match Text::new(
            &format!(
                "  Are you sure you want to claim {} COAL and {} ORE and {} CHROMIUM? (Y/n or 'esc' to cancel)",
                amount_to_ui_amount(balance_grains_coal, coal_api::consts::TOKEN_DECIMALS),
                amount_to_ui_amount(balance_grains_ore, ore_api::consts::TOKEN_DECIMALS),
                amount_to_ui_amount(balance_grains_chromium, coal_api::consts::TOKEN_DECIMALS)
            )
                .red()
                .to_string(),
        )
            .prompt()
        {
            Ok(confirm) => {
                if confirm.trim().eq_ignore_ascii_case("esc") {
                    println!("  Claim canceled.");
                    return;
                } else if confirm.trim().is_empty() || confirm.trim().to_lowercase() == "y" {} else {
                    println!("  Claim canceled.");
                    return;
                }
            }
            Err(InquireError::OperationCanceled) => {
                println!("  Claim operation canceled.");
                return;
            }
            Err(_) => {
                println!("  Invalid input. Claim canceled.");
                return;
            }
        }
    }

    let timestamp = if let Ok(response) = client
        .get(format!("{}://{}/timestamp", url_prefix, url))
        .send()
        .await
    {
        if let Ok(ts) = response.text().await {
            if let Ok(ts) = ts.parse::<u64>() {
                ts
            } else {
                println!("Failed to get timestamp from server, please try again.");
                return;
            }
        } else {
            println!("Failed to get timestamp from server, please try again.");
            return;
        }
    } else {
        println!("Failed to get timestamp from server, please try again.");
        return;
    };

    println!(
        "  Sending claim request for {} COAL and {} ORE and {} CHROMIUM...",
        amount_to_ui_amount(balance_grains_coal, coal_api::consts::TOKEN_DECIMALS),
        amount_to_ui_amount(balance_grains_ore, ore_api::consts::TOKEN_DECIMALS),
        amount_to_ui_amount(balance_grains_chromium, coal_api::consts::TOKEN_DECIMALS)
    );

    let mut signed_msg = vec![];
    signed_msg.extend(timestamp.to_le_bytes());
    signed_msg.extend(receiver_pubkey.to_bytes());
    signed_msg.extend(balance_grains_coal.to_le_bytes());
    signed_msg.extend(balance_grains_ore.to_le_bytes());
    signed_msg.extend(balance_grains_chromium.to_le_bytes());

    let sig = key.sign_message(&signed_msg);
    let auth = BASE64_STANDARD.encode(format!("{}:{}", key.pubkey(), sig));

    let resp = client
        .post(format!(
            "{}://{}/v2/claim?timestamp={}&receiver_pubkey={}&amount_coal={}&amount_ore={}&amount_chromium={}",
            url_prefix,
            url,
            timestamp,
            receiver_pubkey.to_string(),
            balance_grains_coal,
            balance_grains_ore,
            balance_grains_chromium
        ))
        .header("Authorization", format!("Basic {}", auth))
        .send()
        .await;

    match resp {
        Ok(res) => match res.text().await.unwrap().as_str() {
            "SUCCESS" => {
                println!("  Successfully queued claim request!");
            }
            "QUEUED" => {
                println!("  Claim is already queued for processing.");
            }
            other => {
                if let Ok(time) = other.parse::<u64>() {
                    let time_left = 1800 - time;
                    let secs = time_left % 60;
                    let mins = (time_left / 60) % 60;
                    println!(
                        "  You cannot claim until the time is up. Time left until next claim available: {}m {}s",
                        mins, secs
                    );
                } else {
                    println!("  Unexpected response: {}", other);
                }
            }
        },
        Err(e) => {
            println!("  ERROR: {}", e);
            println!("  Retrying in 5 seconds...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
