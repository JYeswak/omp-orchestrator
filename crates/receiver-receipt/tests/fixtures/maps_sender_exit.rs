// Known-bad specimen for 7wn9.5 leg 3. Maps sender exit onto IrcDeliveryReceipt.
fn from_exit(sender_exit: i32) -> IrcDeliveryReceipt {
    IrcDeliveryReceipt {
        to: "x".into(),
        outcome: if sender_exit == 0 {
            IrcDeliveryOutcome::Injected
        } else {
            IrcDeliveryOutcome::Failed
        },
        error: None,
    }
}
