// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! #433: an address that never answers doesn't use up the time the server's other addresses need.

use super::*;

#[test]
fn an_address_that_never_answers_leaves_time_for_the_next() {
    // A simulated clock: each attempt uses up the time it was given, so the test does not depend
    // on what the network does with a test address (it may refuse at once, or time out).
    let clock = std::cell::Cell::new(Duration::ZERO);
    let budget = Duration::from_secs(2);
    let nowhere = SocketAddr::from(([192, 0, 2, 1], 9));
    let here = SocketAddr::from(([127, 0, 0, 1], 9));
    let mut tried = Vec::new();
    let result = connect_resolved(
        &[nowhere, here],
        budget,
        |address, attempt| {
            tried.push(*address);
            if *address == here {
                Ok(())
            } else {
                // The silent address uses up the time it was given, then gives up.
                clock.set(clock.get() + attempt);
                Err(std::io::Error::from(std::io::ErrorKind::TimedOut))
            }
        },
        || clock.get(),
    );
    assert!(result.is_ok(), "the second address was never tried");
    assert_eq!(tried, vec![nowhere, here]);
    assert!(clock.get() < budget, "took {:?}", clock.get());
}

#[test]
fn no_address_says_so() {
    assert_eq!(connect_first(&[], Duration::from_secs(1)).err().map(|e| e.to_string()).as_deref(), Some("no address was found for the name"));
}
