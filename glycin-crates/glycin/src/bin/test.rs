use gdk::prelude::*;
use glycin::*;

fn main() {
    let images = std::fs::read_dir("images").unwrap();

    for entry in images {
        eprintln!("{entry:?}");
        let path = entry.unwrap().path();
        let file = gio::File::for_path(&path);

        async_std::task::block_on(async move {
            let image_request = ImageRequest::new(file);
            let image = image_request.request().await.unwrap();

            let info = image.info();
            let texture = image.next_frame().await.unwrap();

            //dbg!(info);
            //dbg!(&texture);
            let out_path = std::path::PathBuf::from_iter(&["out".into(), path]);
            texture.save_to_png(out_path).unwrap();
        });
    }
}
