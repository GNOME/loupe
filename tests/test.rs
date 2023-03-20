use loupe::{decoder, decoder::tiling, deps::*};

use arc_swap::ArcSwap;
use futures::prelude::*;
use gtk::prelude::*;
use std::sync::Arc;

#[gtk::test]
async fn test1() {
    let file = gio::File::for_path("tests/test.svg");
    let buffer: Arc<ArcSwap<tiling::FrameBuffer>> = Default::default();
    let (decoder, mut decoder_update) = decoder::Decoder::new(file, buffer.clone()).await.unwrap();

    let tile_request = decoder::TileRequest {
        viewport: graphene::Rect::new(0., 0., 600., 600.),
        zoom: 1.,
    };
    decoder.request(tile_request);

    redraw_signal(&mut decoder_update).await;

    let snapshot = gtk::Snapshot::new();
    let render_options = tiling::RenderOptions {
        scaling_filter: gsk::ScalingFilter::Linear,
    };
    buffer
        .load()
        .add_to_snapshot(&snapshot, 1., &render_options);

    debug_render(snapshot);
}

async fn redraw_signal(
    decoder_update: &mut futures::channel::mpsc::UnboundedReceiver<decoder::DecoderUpdate>,
) {
    while let Some(update) = decoder_update.next().await {
        if matches!(update, decoder::DecoderUpdate::Redraw) {
            break;
        }
    }
}

pub fn debug_render(snapshot: gtk::Snapshot) {
    let renderer = gsk::CairoRenderer::new();
    renderer.realize(None).unwrap();

    let node = snapshot.to_node().unwrap();

    let texture = renderer.render_texture(node, None);
    texture.save_to_png("newtest.png").unwrap();

    renderer.unrealize();
}
