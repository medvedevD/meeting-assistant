use meeting_core::entities::TokenUsage;

/// Build a [`TokenUsage`] from the counters a provider put in its response.
/// Every counter is optional on the wire (proxies and local servers omit or
/// null them), and a malformed usage block must never fail a generation that
/// already succeeded, so a missing counter reads as zero and a response with
/// no counters at all yields `None`.
pub(super) fn token_usage(input: &[Option<u32>], output: &[Option<u32>]) -> Option<TokenUsage> {
    if input.iter().chain(output).all(Option::is_none) {
        return None;
    }
    let sum = |counters: &[Option<u32>]| {
        counters
            .iter()
            .flatten()
            .fold(0u32, |acc, n| acc.saturating_add(*n))
    };
    Some(TokenUsage {
        input_tokens: sum(input),
        output_tokens: sum(output),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_every_reported_counter() {
        assert_eq!(
            token_usage(&[Some(10), Some(200), None], &[Some(5), Some(7)]),
            Some(TokenUsage {
                input_tokens: 210,
                output_tokens: 12,
            })
        );
    }

    #[test]
    fn no_counters_means_no_usage() {
        assert_eq!(token_usage(&[None], &[None, None]), None);
    }
}
