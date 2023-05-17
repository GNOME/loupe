use gdk::prelude::*;
use glycin::*;

fn main() {
    let path = "/home/herold/loupetest/DSCN0029-pro.jpg";
    let file = gio::File::for_path(path);

    async_std::task::block_on(async {
        let image_request = ImageRequest::new(file);
        let image = image_request.request().await.unwrap();

        let info = image.info();
        let texture = image.next_frame().await.unwrap();

        dbg!(info);
        dbg!(&texture);
        texture.save_to_png("test-out.png").unwrap();
    });
}
