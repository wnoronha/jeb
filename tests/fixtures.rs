//! Live-verified provider fixture tests.
//!
//! Fixtures in `fixtures/` are recorded from real TypeSafe and OpenRouter
//! calls. These tests assert request and response schemas offline.

use jeb::build_decision_value;
use jeb::types::{Answer, Question, SystemOneRequest, SystemOneResponse};
use jeb::validate_request;

macro_rules! live_fixture_test {
    ($name:ident, $req:expr, $resp:expr, $choice:expr, $noul:expr, $score:expr, $model:expr) => {
        #[test]
        fn $name() {
            assert_live_fixture(
                include_str!($req),
                include_str!($resp),
                $choice,
                $noul,
                $score,
                $model,
            );
        }
    };
}

macro_rules! noul_fixture_test {
    ($name:ident, $req:expr, $resp:expr, $id:expr, $model:expr) => {
        #[test]
        fn $name() {
            assert_noul_fixture(include_str!($req), include_str!($resp), $id, $model);
        }
    };
}

live_fixture_test!(
    test_live_fixture_devops,
    "../fixtures/req_1_devops.json",
    "../fixtures/typesafe/resp_1_devops.json",
    "triage_action",
    "critical_sev",
    "risk",
    "jev-1.13.0"
);
live_fixture_test!(
    test_live_fixture_billing,
    "../fixtures/req_2_billing.json",
    "../fixtures/typesafe/resp_2_billing.json",
    "approval_route",
    "policy_violation",
    "review_priority",
    "jev-1.13.0"
);
live_fixture_test!(
    test_live_fixture_security,
    "../fixtures/req_3_security.json",
    "../fixtures/typesafe/resp_3_security.json",
    "exploitability",
    "needs_hotfix",
    "severity",
    "jev-1.13.0"
);
live_fixture_test!(
    test_openrouter_fixture_devops,
    "../fixtures/req_1_devops.json",
    "../fixtures/openrouter/oresp_1_devops.json",
    "triage_action",
    "critical_sev",
    "risk",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_billing,
    "../fixtures/req_2_billing.json",
    "../fixtures/openrouter/oresp_2_billing.json",
    "approval_route",
    "policy_violation",
    "review_priority",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_security,
    "../fixtures/req_3_security.json",
    "../fixtures/openrouter/oresp_3_security.json",
    "exploitability",
    "needs_hotfix",
    "severity",
    "typesafe/jev-1.13-20260917"
);
noul_fixture_test!(
    test_openrouter_fixture_openai,
    "../fixtures/req_ping.json",
    "../fixtures/openrouter/oresp_openai.json",
    "proceed",
    "openai/gpt-6-luna-decisions-20261006"
);
noul_fixture_test!(
    test_openrouter_fixture_microsoft,
    "../fixtures/req_ping.json",
    "../fixtures/openrouter/oresp_microsoft.json",
    "proceed",
    "microsoft/microsoft-decision-1-20261009"
);
noul_fixture_test!(
    test_openrouter_fixture_cloudflare,
    "../fixtures/req_ping.json",
    "../fixtures/openrouter/oresp_cloudflare.json",
    "proceed",
    "cloudflare/clef"
);
noul_fixture_test!(
    test_typesafe_fixture_ping,
    "../fixtures/req_ping.json",
    "../fixtures/typesafe/resp_ping.json",
    "proceed",
    "jev-1.13.0"
);
live_fixture_test!(
    test_typesafe_fixture_dataeng,
    "../fixtures/req_4_dataeng.json",
    "../fixtures/typesafe/resp_4_dataeng.json",
    "data_action",
    "page_oncall",
    "report_risk",
    "jev-1.13.0"
);
live_fixture_test!(
    test_typesafe_fixture_refund,
    "../fixtures/req_5_refund.json",
    "../fixtures/typesafe/resp_5_refund.json",
    "refund_action",
    "needs_human_review",
    "churn_risk",
    "jev-1.13.0"
);
live_fixture_test!(
    test_typesafe_fixture_flags,
    "../fixtures/req_6_flags.json",
    "../fixtures/typesafe/resp_6_flags.json",
    "rollout_action",
    "within_budget",
    "rollout_risk",
    "jev-1.13.0"
);
live_fixture_test!(
    test_typesafe_fixture_oncall,
    "../fixtures/req_7_oncall.json",
    "../fixtures/typesafe/resp_7_oncall.json",
    "page_action",
    "sev1",
    "customer_impact",
    "jev-1.13.0"
);
live_fixture_test!(
    test_openrouter_fixture_dataeng,
    "../fixtures/req_4_dataeng.json",
    "../fixtures/openrouter/oresp_4_dataeng.json",
    "data_action",
    "page_oncall",
    "report_risk",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_refund,
    "../fixtures/req_5_refund.json",
    "../fixtures/openrouter/oresp_5_refund.json",
    "refund_action",
    "needs_human_review",
    "churn_risk",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_flags,
    "../fixtures/req_6_flags.json",
    "../fixtures/openrouter/oresp_6_flags.json",
    "rollout_action",
    "within_budget",
    "rollout_risk",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_oncall,
    "../fixtures/req_7_oncall.json",
    "../fixtures/openrouter/oresp_7_oncall.json",
    "page_action",
    "sev1",
    "customer_impact",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_typesafe_fixture_ardupilot_battery,
    "../fixtures/req_8_ardupilot_battery.json",
    "../fixtures/typesafe/resp_8_ardupilot_battery.json",
    "flight_action",
    "abort_mission",
    "airframe_risk",
    "jev-1.13.0"
);
live_fixture_test!(
    test_typesafe_fixture_ardupilot_ekf,
    "../fixtures/req_9_ardupilot_ekf.json",
    "../fixtures/typesafe/resp_9_ardupilot_ekf.json",
    "ekf_action",
    "mag_compromised",
    "flyaway_risk",
    "jev-1.13.0"
);
live_fixture_test!(
    test_typesafe_fixture_ardupilot_vibe,
    "../fixtures/req_10_ardupilot_vibe.json",
    "../fixtures/typesafe/resp_10_ardupilot_vibe.json",
    "motor_action",
    "clipping_critical",
    "hardware_severity",
    "jev-1.13.0"
);
live_fixture_test!(
    test_openrouter_fixture_ardupilot_battery,
    "../fixtures/req_8_ardupilot_battery.json",
    "../fixtures/openrouter/oresp_8_ardupilot_battery.json",
    "flight_action",
    "abort_mission",
    "airframe_risk",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_ardupilot_ekf,
    "../fixtures/req_9_ardupilot_ekf.json",
    "../fixtures/openrouter/oresp_9_ardupilot_ekf.json",
    "ekf_action",
    "mag_compromised",
    "flyaway_risk",
    "typesafe/jev-1.13-20260917"
);
live_fixture_test!(
    test_openrouter_fixture_ardupilot_vibe,
    "../fixtures/req_10_ardupilot_vibe.json",
    "../fixtures/openrouter/oresp_10_ardupilot_vibe.json",
    "motor_action",
    "clipping_critical",
    "hardware_severity",
    "typesafe/jev-1.13-20260917"
);

fn assert_live_fixture(
    req_json: &str,
    resp_json: &str,
    choice_id: &str,
    noul_id: &str,
    score_id: &str,
    expected_model: &str,
) {
    let req: SystemOneRequest = serde_json::from_str(req_json).unwrap();
    validate_request(&req).unwrap();
    assert!(matches!(
        req.questions.get(choice_id).unwrap(),
        Question::Choice { .. }
    ));
    assert!(matches!(
        req.questions.get(noul_id).unwrap(),
        Question::Noul { .. }
    ));
    assert!(matches!(
        req.questions.get(score_id).unwrap(),
        Question::Score { .. }
    ));

    let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
    assert_eq!(resp.model, expected_model);
    assert!(matches!(
        resp.answers.get(choice_id).unwrap(),
        Answer::Choice { .. }
    ));
    assert!(matches!(
        resp.answers.get(noul_id).unwrap(),
        Answer::Noul { .. }
    ));
    assert!(matches!(
        resp.answers.get(score_id).unwrap(),
        Answer::Score { .. }
    ));
    let summary = build_decision_value(&resp);
    assert_eq!(summary["decisions"][choice_id]["type"], "choice");
    assert_eq!(summary["decisions"][noul_id]["type"], "noul");
    assert_eq!(summary["decisions"][score_id]["type"], "score");
    assert!(summary["decisions"][score_id]["decision"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
}

fn assert_noul_fixture(req_json: &str, resp_json: &str, id: &str, expected_model: &str) {
    let req: SystemOneRequest = serde_json::from_str(req_json).unwrap();
    validate_request(&req).unwrap();
    assert!(matches!(
        req.questions.get(id).unwrap(),
        Question::Noul { .. }
    ));

    let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
    assert_eq!(resp.model, expected_model);
    match resp.answers.get(id).unwrap() {
        Answer::Noul { noul } => assert!((0.0..=1.0).contains(noul)),
        _ => panic!("Expected noul answer"),
    }
    let summary = build_decision_value(&resp);
    assert_eq!(summary["decisions"][id]["type"], "noul");
}
