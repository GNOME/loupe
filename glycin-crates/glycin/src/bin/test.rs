use glycin::*;

fn main() {
    let path = "/home/herold/loupetest/DSCN0029.jpg";
    let file = gio::File::for_path(path);

    async_std::task::block_on(async {
        let image_request = ImageRequest::new(file);
        let image = image_request.request().await;

        let height = image.info().height;
        let texture = image.next_frame().await;
    });
}
