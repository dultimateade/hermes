use proptest::prelude::*;

use predictify_hybrid::leaderboard::{key_win_rate, key_winnings};
use predictify_hybrid::types::UserLeaderboardEntryV1;
use soroban_sdk::{Address, Env};

fn entry(
    env: &Env,
    winnings: i128,
    win_rate: u32,
    bets: u64,
) -> UserLeaderboardEntryV1 {
    UserLeaderboardEntryV1 {
        user: Address::generate(env),
        rank: 0,
        total_winnings: winnings,
        win_rate,
        total_bets_placed: bets,
        winning_bets: 0,
        total_wagered: 0,
        last_activity: 0,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn winnings_ranking_is_stable(
        a in -1_000_000i128..=1_000_000,
        b in -1_000_000i128..=1_000_000,
    ) {
        let env = Env::default();
        let ea = entry(&env, a, 5000, 10);
        let eb = entry(&env, b, 5000, 10);
        prop_assert_eq!(key_winnings(&ea) >= key_winnings(&eb), a >= b);
        prop_assert_eq!(key_winnings(&ea) == key_winnings(&eb), a == b);
    }

    #[test]
    fn win_rate_ranking_is_transitive(
        a in 0u32..=10_000,
        b in 0u32..=10_000,
        c in 0u32..=10_000,
        abets in 0u64..=1_000,
        bbets in 0u64..=1_000,
        cbets in 0u64..=1_000,
    ) {
        let env = Env::default();
        let ea = entry(&env, 0, a, abets);
        let eb = entry(&env, 0, b, bbets);
        let ec = entry(&env, 0, c, cbets);
        let ka = key_win_rate(&ea);
        let kb = key_win_rate(&eb);
        let kc = key_win_rate(&ec);
        if ka >= kb && kb >= kc {
            prop_assert!(ka >= kc);
        }
    }

    #[test]
    fn equal_win_rate_uses_bet_volume_as_tiebreaker(
        bets_a in 0u64..=1_000,
        bets_b in 0u64..=1_000,
    ) {
        let env = Env::default();
        let ea = entry(&env, 100, 5000, bets_a);
        let eb = entry(&env, 200, 5000, bets_b);
        prop_assert_eq!(
            key_win_rate(&ea) >= key_win_rate(&eb),
            bets_a >= bets_b
        );
    }
}
