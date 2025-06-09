use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolandElectionMarkets {
    pub election_date: String,
    pub candidate_markets: Vec<CandidateMarket>,
    pub turnout_markets: Vec<TurnoutMarket>,
    pub other_markets: Vec<Market>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateMarket {
    pub candidate_name: String,
    pub question: String,
    pub condition_id: String,
    pub market_slug: String,
    pub active: bool,
    pub current_probability: f64,
    pub yes_token: Token,
    pub no_token: Token,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnoutMarket {
    pub turnout_range: String,
    pub question: String,
    pub condition_id: String,
    pub active: bool,
    pub current_probability: f64,
    pub yes_token: Token,
    pub no_token: Token,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub question: String,
    pub condition_id: String,
    pub market_slug: String,
    pub active: bool,
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub outcome: String,
    pub token_id: String,
    pub current_price: f64,
}

/// Get all Poland Presidential Election 2025 markets
#[allow(dead_code)]
pub fn get_poland_election_markets() -> PolandElectionMarkets {
    PolandElectionMarkets {
        election_date: "May 2025".to_string(),
        candidate_markets: vec![
            CandidateMarket {
                candidate_name: "Rafał Trzaskowski".to_string(),
                question: "Will Rafał Trzaskowski be the next President of Poland?".to_string(),
                condition_id: "0xd009ac14bccdd12925a2d9f8d910411556e4ed153337abb18a97b97fabcf7db0".to_string(),
                market_slug: "will-rafa-trzaskowski-be-the-next-president-of-poland".to_string(),
                active: true,
                current_probability: 0.6425,
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "9985510571211594606436989364549728150268400968429116207816527248582274291346".to_string(),
                    current_price: 0.6425,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "95044204495071447962354072933210357509612220350121617225795580441278155097858".to_string(),
                    current_price: 0.3575,
                },
            },
            CandidateMarket {
                candidate_name: "Karol Nawrocki".to_string(),
                question: "Will Karol Nawrocki be the next President of Poland?".to_string(),
                condition_id: "0x5ce0d897bd66142c43a38204a67ad85bc3e0643382258411a5aa58ca3e825082".to_string(),
                market_slug: "will-karol-nawrocki-be-the-next-president-of-poland".to_string(),
                active: true,
                current_probability: 0.3555,
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "52378310446953465163845338048369876961360578335284428810587513450420811578746".to_string(),
                    current_price: 0.3555,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "12000674920991755992211074758866949296981843307621554385852540153569787669909".to_string(),
                    current_price: 0.6445,
                },
            },
            CandidateMarket {
                candidate_name: "Szymon Hołownia".to_string(),
                question: "Will Szymon Hołownia be the next President of Poland?".to_string(),
                condition_id: "0x9067b99e22ed2b8ed4bad7d72ce8eed3706861ae2c3a79f769e13de7c2967e6c".to_string(),
                market_slug: "will-szymon-hoownia-be-the-next-president-of-poland".to_string(),
                active: true,
                current_probability: 0.0, // Price not shown in data, needs update
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "74068535019060214167141032054373162823046783650508628209900128734066256193284".to_string(),
                    current_price: 0.0,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "47080901571753401590346134208580914055615816978058982060238958992385631696291".to_string(),
                    current_price: 1.0,
                },
            },
            CandidateMarket {
                candidate_name: "Marek Jakubiak".to_string(),
                question: "Will Marek Jakubiak be the next President of Poland?".to_string(),
                condition_id: "0x95874e7c989d91e5268ed7e880acb9269f13803ea0b5629f4a6acd1a4b5d6fbb".to_string(),
                market_slug: "will-marek-jakubiak-be-the-next-president-of-poland".to_string(),
                active: true,
                current_probability: 0.0,
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "96912414706418192981807917566359563852315470166366122237403501995199388891594".to_string(),
                    current_price: 0.0,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "21980654457646845991938014937177283296130471384499160909316172508498614677160".to_string(),
                    current_price: 1.0,
                },
            },
        ],
        turnout_markets: vec![
            TurnoutMarket {
                turnout_range: "62-64%".to_string(),
                question: "Will voter turnout in the 2025 Polish presidential election be between 62-64%?".to_string(),
                condition_id: "0xb33d5c42f9e1595003584dd8fdc625d78cba312137b9b0dea2e5fed46938eeb8".to_string(),
                active: true,
                current_probability: 0.004,
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "26059829235404409463657325826177946721830054241914428155374720991180873913989".to_string(),
                    current_price: 0.004,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "43235212041236424670666929755608554583123143118523700166942749520129474671531".to_string(),
                    current_price: 0.996,
                },
            },
            TurnoutMarket {
                turnout_range: "70-72%".to_string(),
                question: "Will voter turnout in the 2025 Polish presidential election be between 70-72%?".to_string(),
                condition_id: "0xb081b826e9838fb4eaeeaf372c1e6504e3028dcaee9478603c9a0ebd3e315fc7".to_string(),
                active: true,
                current_probability: 0.265,
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "38002064998644428921886819930918992347431883586358733138272927690341664589016".to_string(),
                    current_price: 0.265,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "55845876904311399326294665991667179396782126813234385066323754692590719241827".to_string(),
                    current_price: 0.735,
                },
            },
            TurnoutMarket {
                turnout_range: "More than 72%".to_string(),
                question: "Will voter turnout in the 2025 Polish presidential election be more than 72%?".to_string(),
                condition_id: "0xd3505ebab4f2d6496a0c1451a21584d95a0fae3d06c948ed42c373c59fd63e5a".to_string(),
                active: true,
                current_probability: 0.59,
                yes_token: Token {
                    outcome: "Yes".to_string(),
                    token_id: "25687965354506026206109599465014803168005382362883880593389835693451852547284".to_string(),
                    current_price: 0.59,
                },
                no_token: Token {
                    outcome: "No".to_string(),
                    token_id: "16268364035266065991322126662304077974642508071104857010747211565242852936936".to_string(),
                    current_price: 0.41,
                },
            },
        ],
        other_markets: vec![
            Market {
                question: "Will Rafał Trzaskowski win the most votes in the first round of the Polish Presidential election?".to_string(),
                condition_id: "0xf8a8cd4602a05b6dd7a87c0f793544e2a8db6d0d1ffc7b600d933399651469a3".to_string(),
                market_slug: "will-rafa-trzaskowski-win-the-most-votes-in-the-first-round-of-the-polish-presidential-election".to_string(),
                active: true,
                tokens: vec![
                    Token {
                        outcome: "Yes".to_string(),
                        token_id: "49359597556653434648739280557982921944912823311218825642413551151246271247982".to_string(),
                        current_price: 1.0,
                    },
                    Token {
                        outcome: "No".to_string(),
                        token_id: "0".to_string(), // This needs to be found
                        current_price: 0.0,
                    },
                ],
            },
        ],
    }
}

/// Export markets to JSON file
#[allow(dead_code)]
pub fn export_to_json(filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let markets = get_poland_election_markets();
    let json = serde_json::to_string_pretty(&markets)?;
    std::fs::write(filename, json)?;
    Ok(())
}

/// Load markets from JSON file
#[allow(dead_code)]
pub fn load_from_json(filename: &str) -> Result<PolandElectionMarkets, Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(filename)?;
    let markets: PolandElectionMarkets = serde_json::from_str(&json)?;
    Ok(markets)
} 