use gdk::prelude::*;
use glycin::*;

fn main() {
    let images = std::fs::read_dir("images").unwrap();

    for entry in images {
        eprintln!("{entry:?}");
        let path = entry.unwrap().path();
        let file = gio::File::for_path(&path);

        async_std::task::block_on(async move {
            let cancellable = gio::Cancellable::new();
            let image_request = ImageRequest::new(file).cancellable(cancellable);

            let image = image_request.request().await.unwrap();
            let texture = image.next_frame().await.unwrap();

            let mut extension = path.extension().unwrap().to_os_string();
            extension.push(".png");
            let out_path =
                std::path::PathBuf::from_iter(&["out".into(), path.with_extension(extension)]);

            texture.save_to_png(out_path).unwrap();
        });
    }
}
