use clap::Parser;
use solana_sdk::signer::Signer;

#[derive(Debug, Parser)]
pub struct StakeToPoolArgs {
    #[arg(long, value_name = "AMOUNT", help = "Amount of LP to stake.")]
    pub amount: f64,
}
/*pub async fn stake_to_pool(args: StakeToPoolArgs, key: Keypair, url: String, unsecure: bool) {
    let base_url = url;
    let client = reqwest::Client::new();
    let url_prefix = if unsecure {
        "http".to_string()
    } else {
        "https".to_string()
    };
    let balance = get_token_balance(
        &key.pubkey(),
        base_url.clone(),
        unsecure,
        COAL_MINT_ADDRESS.to_string(),
    )
    .await;

    // Ensure stake amount does not exceed balance
    let stake_amount = if args.amount > balance {
        println!(
            "  You do not have enough COAL to stake {} to the pool.\n  Adjusting stake amount to the maximum available: {} COAL",
            args.amount, balance
        );
        balance
    } else {
        args.amount
    };

    // RED TEXT
    match Text::new(
        &format!(
            "  Are you sure you want to stake {} COAL? (Y/n or 'esc' to cancel)",
            stake_amount
        )
        .red()
        .to_string(),
    )
    .prompt()
    {
        Ok(confirm) => {
            if confirm.trim().eq_ignore_ascii_case("esc") {
                println!("  Pool staking canceled.");
                return;
            } else if confirm.trim().is_empty() || confirm.trim().to_lowercase() == "y" {
                // Proceed with staking
            } else {
                println!("  Pool staking canceled.");
                return;
            }
        }
        Err(InquireError::OperationCanceled) => {
            println!("  Pool staking operation canceled.");
            return;
        }
        Err(_) => {
            println!("  Invalid input. Pool staking canceled.");
            return;
        }
    }

    let resp = client
        .get(format!(
            "{}://{}/pool/fee_payer/pubkey",
            url_prefix, base_url
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let fee_pubkey = Pubkey::from_str(&resp).unwrap();

    let resp = client
        .get(format!(
            "{}://{}/pool/authority/pubkey",
            url_prefix, base_url
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let pool_pubkey = Pubkey::from_str(&resp).unwrap();

    // Subscribe the miner to the pool if needed
    let resp = client
        .post(format!(
            "{}://{}/v2/signup?miner={}",
            url_prefix,
            base_url,
            key.pubkey().to_string(),
        ))
        .body("BLANK".to_string())
        .send()
        .await;
    if let Ok(res) = resp {
        if let Ok(txt) = res.text().await {
            match txt.as_str() {
                "SUCCESS" => {
                    println!("  Successfully signed up!");
                }
                "EXISTS" => {}
                _ => {
                    println!("  Signup transaction failed, please try again.");
                }
            }
        } else {
            println!("  Signup transaction failed, please wait and try again.");
        }
    } else {
        println!("  Signup transaction failed, please wait and try again.");
    }

    let stake_amount_u64 = (stake_amount * 10f64.powf(TOKEN_DECIMALS as f64)) as u64;

    println!("stake_amount_u64 {}", stake_amount_u64);

    // we have all the basic info, let's start building the transaction
    let mut ixs: Vec<Instruction> = vec![];

    let user_token_account_coal = get_associated_token_address(&key.pubkey(), &COAL_MINT_ADDRESS);
    let pool_token_account_coal = get_associated_token_address(&pool_pubkey, &COAL_MINT_ADDRESS);

    println!("pool_token_account_coal: {}", pool_token_account_coal);

    match spl_token::instruction::transfer(
        &spl_token::id(),
        &user_token_account_coal,
        &pool_token_account_coal,
        &key.pubkey(),
        &[],
        stake_amount_u64,
    ) {
        Ok(ix) => {
            ixs.push(ix);
        }
        Err(e) => {
            println!("Error transfer: {}", e);
            return;
        }
    }

    let resp = client
        .get(format!("{}://{}/latest-blockhash", url_prefix, base_url))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let decoded_blockhash = BASE64_STANDARD.decode(resp).unwrap();
    let deserialized_blockhash = bincode::deserialize(&decoded_blockhash).unwrap();

    let mut tx = Transaction::new_with_payer(&ixs, Some(&fee_pubkey));

    tx.partial_sign(&[&key], deserialized_blockhash);
    let serialized_tx = bincode::serialize(&tx).unwrap();
    let encoded_tx = BASE64_STANDARD.encode(&serialized_tx);

    let resp = client
        .post(format!(
            "{}://{}/coal/stake?pubkey={}&amount={}",
            url_prefix,
            base_url,
            key.pubkey().to_string(),
            stake_amount_u64
        ))
        .body(encoded_tx)
        .send()
        .await;
    if let Ok(res) = resp {
        if let Ok(txt) = res.text().await {
            match txt.as_str() {
                "SUCCESS" => {
                    println!("  Successfully staked to pool! The staked COAL got added to your total balance");
                }
                other => {
                    println!("  Transaction failed: {}", other);
                }
            }
        } else {
            println!("  Transaction failed, please wait and try again.");
        }
    } else {
        println!("  Transaction failed, please wait and try again.");
    }
}
*/
