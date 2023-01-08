use axum::{extract::Path, response::Redirect};

pub async fn redirect_osudirect(Path(mapset_id): Path<u32>) -> Redirect {
    let location = format!("osu://dl/{mapset_id}");

    Redirect::permanent(&location)
}
