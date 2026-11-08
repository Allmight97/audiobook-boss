use abb_audible_core::{library_items_from_page, LibraryItem};
use serde_json::Value;

use crate::remote_source::{
    ProviderId, RemoteTitle, RemoteTitleAvailability, RemoteTitleAvailabilityStatus,
};

pub(super) fn parse_library_titles(payload: &Value) -> Vec<RemoteTitle> {
    library_items_from_page(payload)
        .into_iter()
        .map(remote_title)
        .collect()
}

fn remote_title(item: LibraryItem) -> RemoteTitle {
    RemoteTitle {
        provider_id: ProviderId::Audible,
        availability: title_availability(&item),
        title_id: item.title_id,
        title: item
            .title
            .unwrap_or_else(|| "Untitled Audible title".to_string()),
        authors: item.authors,
        narrators: item.narrators,
        cover: item
            .cover_url
            .as_deref()
            .map(crate::session::cover_remote_address),
        supplemental_pdf_available: item.supplemental_pdf_available,
    }
}

fn title_availability(item: &LibraryItem) -> RemoteTitleAvailability {
    if item.revoked {
        return RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::Revoked,
            acquirable: false,
            label: "Returned/refunded in Audible".to_string(),
            detail: Some(
                "Audible reports this title is no longer playable or downloadable for this account."
                    .to_string(),
            ),
        };
    }

    if item.playable == Some(false) {
        if item.ayce == Some(true) {
            return RemoteTitleAvailability {
                status: RemoteTitleAvailabilityStatus::CatalogOnly,
                acquirable: false,
                label: "Audible catalog title".to_string(),
                detail: Some(
                    "Audible reports this title is not downloadable for this account.".to_string(),
                ),
            };
        }
        return RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::ProviderUnavailable,
            acquirable: false,
            label: "Unavailable from Audible".to_string(),
            detail: Some(
                "Audible reports this title is not playable or downloadable for this account."
                    .to_string(),
            ),
        };
    }

    RemoteTitleAvailability {
        status: RemoteTitleAvailabilityStatus::Available,
        acquirable: true,
        label: "Available".to_string(),
        detail: None,
    }
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
