//! Pure rendering of decision summaries for `jeb`.

use anyhow::Result;

use crate::build_decision_value;
use crate::types::SystemOneResponse;

/// Builds the top decision summary as a JSON string.
///
/// Maps a noul probability at or above 0.5 to yes.
/// Maps a lower probability to no.
pub fn render_summary(response: &SystemOneResponse, pretty: bool) -> Result<String> {
    let out = build_decision_value(response);

    if pretty {
        Ok(serde_json::to_string_pretty(&out)?)
    } else {
        Ok(serde_json::to_string(&out)?)
    }
}
