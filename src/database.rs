use std::{path::Path, sync::RwLock};

use rusqlite::Connection;

pub struct PoolSubmissionResult {
    id: i32,
    pool_difficulty: u32,
    pool_earned_coal: u64,
    pool_earned_ore: u64,
    miner_percentage_coal: f64,
    miner_percentage_ore: f64,
    miner_difficulty: u32,
    miner_earned_coal: u64,
    miner_earned_ore: u64,
    created_at: u64,
}

impl PoolSubmissionResult {
    pub fn new(
        pool_difficulty: u32,
        pool_earned_coal: u64,
        pool_earned_ore: u64,
        miner_percentage_coal: f64,
        miner_percentage_ore: f64,
        miner_difficulty: u32,
        miner_earned_coal: u64,
        miner_earned_ore: u64,
    ) -> Self {
        PoolSubmissionResult {
            id: 0,
            pool_difficulty,
            pool_earned_coal,
            pool_earned_ore,
            miner_percentage_coal,
            miner_percentage_ore,
            miner_difficulty,
            miner_earned_coal,
            miner_earned_ore,
            created_at: 0,
        }
    }
}

pub struct AppDatabase {
    connection: RwLock<Connection>,
}

impl AppDatabase {
    pub fn new() -> Self {
        let conn = match Connection::open(Path::new("./app_db_merged.db3")) {
            Ok(c) => {
                match c.execute(
                    r#"CREATE TABLE IF NOT EXISTS pool_submission_results (
                        id INTEGER PRIMARY KEY,
                        pool_difficulty INTEGER NOT NULL,
                        pool_earned_coal INTEGER NOT NULL,
                        pool_earned_ore INTEGER NOT NULL,
                        miner_percentage_coal NUMERIC NOT NULL,
                        miner_percentage_ore NUMERIC NOT NULL,
                        miner_difficulty INTEGER NOT NULL,
                        miner_earned_coal INTEGER NOT NULL,
                        miner_earned_ore INTEGER NOT NULL,
                        created_at  INTEGER DEFAULT CURRENT_TIMESTAMP NOT NULL
                    )"#,
                    (),
                ) {
                    Ok(_) => {
                        c
                    }
                    Err(e) => {
                        eprintln!("Error creating pool_submission_results table!");
                        panic!("Error: {e}");
                    }
                }
            }
            Err(_e) => {
                panic!("Failed to open app database");
            }
        };
        AppDatabase {
            connection: RwLock::new(conn),
        }
    }

    pub fn add_new_pool_submission(&self, new_pool_submission_result: PoolSubmissionResult) {
        if let Err(e) = self.connection.write().unwrap().execute(
            r#"INSERT INTO pool_submission_results (
                pool_difficulty,
                pool_earned_coal,
                pool_earned_ore,
                miner_percentage_coal,
                miner_percentage_ore,
                miner_difficulty,
                miner_earned_coal,
                miner_earned_ore
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            (
                &new_pool_submission_result.pool_difficulty,
                &new_pool_submission_result.pool_earned_coal,
                &new_pool_submission_result.pool_earned_ore,
                &new_pool_submission_result.miner_percentage_coal,
                &new_pool_submission_result.miner_percentage_ore,
                &new_pool_submission_result.miner_difficulty,
                &new_pool_submission_result.miner_earned_coal,
                &new_pool_submission_result.miner_earned_ore,
            ),
        ) {
            eprintln!("Error: Failed to insert pool submission result.\nE: {e}");
        }
    }

    pub fn get_todays_earnings_coal(&self) -> u64 {
        match self.connection.write().unwrap().prepare(
            r#"SELECT SUM(miner_earned_coal) as total_earned
               FROM pool_submission_results
               WHERE created_at >= date('now', 'start of day')
            "#
        ) {
            Ok(mut stmt) => {
                let total_earned: Option<u64> = stmt.query_row([], |row| row.get(0)).unwrap();
                match total_earned {
                    Some(sum) => {
                        return sum
                    }
                    None => {
                        return 0
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: Failed to get todays earnings.\nE: {e}");
                return 0;
            }
        }
    }

    pub fn get_daily_earnings_coal(&self, days: u32) -> Vec<(String, u64)> {
        match self.connection.write().unwrap().prepare(
            r#"SELECT DATE(created_at) as day,SUM(miner_earned_coal) as total_earned
               FROM pool_submission_results
               WHERE created_at >= date('now', '-6 days')
               GROUP BY DATE(created_at)
               ORDER BY DATE(created_at)
            "#
        ) {
            Ok(mut stmt) => {
                let earnings_iter = stmt.query_map([], |row| {
                    let day: String = row.get(0).unwrap();
                    let total_earned: u64 = row.get(1).unwrap();
                    Ok((day, total_earned))
                }).unwrap();

                let mut earnings = vec![];
                for earning in earnings_iter {
                    match earning {
                        Ok((day, total_earned)) => {
                            earnings.push((day, total_earned));
                        }
                        Err(_) => {
                            eprintln!("Error getting earning");
                        }
                    }
                }

                return earnings;
            }
            Err(e) => {
                eprintln!("Error: Failed to get todays earnings.\nE: {e}");
                return vec![];
            }
        }
    }

    pub fn get_todays_earnings_ore(&self) -> u64 {
        match self.connection.write().unwrap().prepare(
            r#"SELECT SUM(miner_earned_ore) as total_earned
               FROM pool_submission_results
               WHERE created_at >= date('now', 'start of day')
            "#
        ) {
            Ok(mut stmt) => {
                let total_earned: Option<u64> = stmt.query_row([], |row| row.get(0)).unwrap();
                match total_earned {
                    Some(sum) => {
                        return sum
                    }
                    None => {
                        return 0
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: Failed to get todays earnings.\nE: {e}");
                return 0;
            }
        }
    }

    pub fn get_daily_earnings_ore(&self, days: u32) -> Vec<(String, u64)> {
        match self.connection.write().unwrap().prepare(
            r#"SELECT DATE(created_at) as day,SUM(miner_earned_ore) as total_earned
               FROM pool_submission_results
               WHERE created_at >= date('now', '-6 days')
               GROUP BY DATE(created_at)
               ORDER BY DATE(created_at)
            "#
        ) {
            Ok(mut stmt) => {
                let earnings_iter = stmt.query_map([], |row| {
                    let day: String = row.get(0).unwrap();
                    let total_earned: u64 = row.get(1).unwrap();
                    Ok((day, total_earned))
                }).unwrap();

                let mut earnings = vec![];
                for earning in earnings_iter {
                    match earning {
                        Ok((day, total_earned)) => {
                            earnings.push((day, total_earned));
                        }
                        Err(_) => {
                            eprintln!("Error getting earning");
                        }
                    }
                }

                return earnings;
            }
            Err(e) => {
                eprintln!("Error: Failed to get todays earnings.\nE: {e}");
                return vec![];
            }
        }
    }
}
