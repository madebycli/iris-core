//! Intel production-root fixture distributed by dcap-qvl 0.7.1 (MIT, see fixtures/DCAP-LICENSE).
//! Time is frozen ONLY in this fixture test. Production uses SystemTime and rejects stale collateral.
use dcap_qvl::{verify::rustcrypto::verify, QuoteCollateralV3};

#[test]
fn verifies_real_intel_quote_and_rejects_tampering_and_expiry() {
    let quote = include_bytes!("fixtures/tdx_quote");
    let collateral: QuoteCollateralV3 =
        serde_json::from_slice(include_bytes!("fixtures/tdx_quote_collateral.json")).unwrap();
    let fixture_time = 1_752_000_000;
    let verified = verify(quote, &collateral, fixture_time).unwrap();
    assert_eq!(verified.status, "UpToDate");
    assert!(verified.report.as_td10().is_some());
    let mut tampered = quote.to_vec();
    tampered[48 + 520] ^= 1; // enclave REPORTDATA, covered by the Intel quote signature
    assert!(verify(&tampered, &collateral, fixture_time).is_err());
    assert!(verify(quote, &collateral, 2_000_000_000).is_err());
}
