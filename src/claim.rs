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
    #[arg(
        long,
        short,
        action,
        help = "Auto approve confirmations."
    )]
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
        Some(rpk) => {
            match Pubkey::from_str(&rpk) {
                Ok(pk) => {
                    pk
                },
                Err(_) => {
                    println!("Failed to parse provided receiver pubkey.\nDouble check the provided public key is valid and try again.");
                    return
                }
            }
        },
        None => {
            key.pubkey()
        }
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
        .unwrap()
        .text()
        .await
        .unwrap();

    let balance = if let Ok(parsed_balance) = balance_response.parse::<f64>() {
        parsed_balance
    } else {
        // If the wallet balance failed to parse
        println!("\n  Note: A 0.004 COAL fee will be deducted from your claim amount to cover the cost\n  of Token Account Creation. This is a one time fee used to create the COAL Token Account.");
        0.0
    };

    let rewards_response = client
        .get(format!(
            "{}://{}/miner/rewards?pubkey={}",
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

    let rewards = rewards_response.parse::<f64>().unwrap_or(0.0);

    println!("  Miner Unclaimed Rewards:      {:.11} COAL", rewards);
    println!("  Receiving Wallet Coal Balance: {:.11} COAL", balance);

    let minimum_claim_amount = 1.0;
    if rewards < minimum_claim_amount {
        println!();
        println!("  You have not reached the required claim limit of 1 COAL.");
        println!("  Keep mining to accumulate more rewards before you can withdraw.");
        return;
    }

    // Convert balance to grains
    let balance_grains = (rewards * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64)) as u64;

    // If balance is zero, inform the user and return to keypair selection
    if balance_grains == 0 {
        println!("\n  There is no balance to claim.");
        return;
    }

    let mut claim_amount = args.amount.unwrap_or(rewards);

    // Prompt the user for an amount if it's not provided or less than 1
    loop {
        if claim_amount < minimum_claim_amount {
            if claim_amount != 0.0 {
                // Only show the message if they previously entered an invalid value
                println!("  Please enter a number above 1.");
            }

            match Text::new("\n  Enter the amount to claim (minimum 0.1 COAL or 'esc' to cancel):")
                .prompt()
            {
                Ok(input) => {
                    if input.trim().eq_ignore_ascii_case("esc") {
                        println!("  Claim operation canceled.");
                        return;
                    }

                    claim_amount = match input.trim().parse::<f64>() {
                        Ok(val) if val >= 0.1 => val,
                        _ => {
                            println!("  Please enter a valid number above 0.1.");
                            continue;
                        }
                    };
                }
                Err(InquireError::OperationCanceled) => {
                    println!("  Claim operation canceled.");
                    return;
                }
                Err(_) => {
                    println!("  Invalid input. Please try again.");
                    continue;
                }
            }
        } else {
            break;
        }
    }

    // Convert the claim amount to the smallest unit
    let mut claim_amount_grains =
        (claim_amount * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64)) as u64;

    // Auto-adjust the claim amount if it exceeds the available balance
    if claim_amount_grains > balance_grains {
        println!(
            "  You do not have enough rewards to claim {} COAL.",
            amount_to_ui_amount(claim_amount_grains, coal_api::consts::TOKEN_DECIMALS)
        );
        claim_amount_grains = balance_grains;
        println!(
            "  Adjusting claim amount to the maximum available: {} COAL.",
            amount_to_ui_amount(claim_amount_grains, coal_api::consts::TOKEN_DECIMALS)
        );
    }

    // RED TEXT
    if !args.y {
        match Text::new(
            &format!(
                "  Are you sure you want to claim {} COAL? (Y/n or 'esc' to cancel)",
                amount_to_ui_amount(claim_amount_grains, coal_api::consts::TOKEN_DECIMALS)
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
                } else if confirm.trim().is_empty() || confirm.trim().to_lowercase() == "y" {
                } else {
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
        "  Sending claim request for {} COAL...",
        amount_to_ui_amount(claim_amount_grains, coal_api::consts::TOKEN_DECIMALS)
    );

    let mut signed_msg = vec![];
    signed_msg.extend(timestamp.to_le_bytes());
    signed_msg.extend(receiver_pubkey.to_bytes());
    signed_msg.extend(claim_amount_grains.to_le_bytes());

    let sig = key.sign_message(&signed_msg);
    let auth = BASE64_STANDARD.encode(format!("{}:{}", key.pubkey(), sig));

    let resp = client
        .post(format!(
            "{}://{}/v2/claim?timestamp={}&receiver_pubkey={}&amount={}",
            url_prefix,
            url,
            timestamp,
            receiver_pubkey.to_string(),
            claim_amount_grains
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
