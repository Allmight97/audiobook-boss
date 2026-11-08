pub(super) mod audible;
pub(super) mod audnexus;
pub(super) mod openlibrary;

const AUDNEXUS_BASE_URL: &str = "https://api.audnex.us";
const AUDIBLE_BASE_URL: &str = "https://api.audible.{tld}";
const OPENLIBRARY_BASE_URL: &str = "https://openlibrary.org";

#[derive(Clone, Debug)]
pub(in crate::metadata_lookup) struct ProviderEndpoints {
    audnexus: String,
    audible: String,
    openlibrary: String,
}

impl ProviderEndpoints {
    pub(in crate::metadata_lookup) fn production() -> Self {
        Self::at(AUDNEXUS_BASE_URL, AUDIBLE_BASE_URL, OPENLIBRARY_BASE_URL)
    }

    pub(in crate::metadata_lookup) fn at(
        audnexus: impl Into<String>,
        audible: impl Into<String>,
        openlibrary: impl Into<String>,
    ) -> Self {
        Self {
            audnexus: audnexus.into(),
            audible: audible.into(),
            openlibrary: openlibrary.into(),
        }
    }

    fn audible_for(&self, tld: &str) -> String {
        self.audible.replace("{tld}", tld)
    }
}
