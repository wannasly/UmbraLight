use lightgui_core::subscription::fetch_subscription;

#[tokio::main]
async fn main() {
    println!("=== Testing Invalid Subscription URLs ===");

    // Test 1: Empty URL
    let res = fetch_subscription("", None, None).await;
    match res {
        Err(e) => println!("  PASS: Empty URL rejected: {e}"),
        Ok(_) => panic!("Empty URL unexpectedly succeeded!"),
    }

    // Test 2: Completely malformed URL
    let res = fetch_subscription("not_a_valid_url_scheme", None, None).await;
    match res {
        Err(e) => println!("  PASS: Malformed URL rejected: {e}"),
        Ok(_) => panic!("Malformed URL unexpectedly succeeded!"),
    }

    // Test 3: Unreachable domain / connection failure
    let res = fetch_subscription("http://127.0.0.1:59999/nonexistent_sub", None, None).await;
    match res {
        Err(e) => println!("  PASS: Unreachable endpoint correctly failed: {e}"),
        Ok(_) => panic!("Unreachable endpoint unexpectedly succeeded!"),
    }

    println!("\nAll subscription edge cases handled safely without panics!");
}
