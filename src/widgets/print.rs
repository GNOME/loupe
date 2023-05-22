// Copyright (c) 2023 Sophie Herold
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The print layout widget that also coordinates the print process

use crate::decoder;
use crate::deps::*;
use crate::widgets::{LpImage, LpPrintPreview};

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Properties;
use gtk::CompositeTemplate;
use once_cell::sync::OnceCell;

use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(file = "../../data/gtk/print.ui")]
    #[properties(wrapper_type = super::LpPrint)]
    pub struct LpPrint {
        #[template_child]
        pub(super) preview: TemplateChild<LpPrintPreview>,

        // ListBox entries
        #[template_child]
        pub(super) width: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub(super) scale: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) margin_left: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub(super) margin_top: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub(super) unit: TemplateChild<adw::ComboRow>,

        #[property(get, set, builder().construct_only())]
        pub(super) image: OnceCell<LpImage>,
        #[property(get, set, builder().construct_only())]
        pub(super) parent_window: OnceCell<gtk::Window>,
        #[property(get, set, builder().construct_only())]
        pub(super) print_settings: OnceCell<gtk::PrintSettings>,
        #[property(get)]
        pub(super) print_operation: gtk::PrintOperation,

        #[property(get, set)]
        pub(super) orientation: RefCell<String>,
        #[property(get, set)]
        pub(super) unit_factor: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpPrint {
        const NAME: &'static str = "LpPrint";
        type Type = super::LpPrint;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);

            klass.install_action_async("print.print", None, |print, _, _| async move {
                print.print();
            });

            klass.install_action_async("print.back", None, |print, _, _| async move {
                print.back().await;
            });

            klass.install_action("print.center-horizontally", None, |print, _, _| {
                print.center_horizontally();
            });

            klass.install_action("print.center-vertically", None, |print, _, _| {
                print.center_vertically();
            });

            klass.install_property_action("print.orientation", "orientation");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpPrint {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.set_unit_factor(1.);

            obj.set_transient_for(Some(&obj.parent_window()));
            obj.set_modal(true);

            self.print_operation
                .set_print_settings(Some(&obj.print_settings()));
            self.print_operation.set_allow_async(true);
            self.print_operation.set_n_pages(1);
            self.print_operation.set_current_page(0);

            if let Some(uri) = obj.image().file().map(|x| x.uri()) {
                obj.print_settings()
                    .set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&format!("{uri}.pdf")));
            }

            self.print_operation.connect_begin_print(move |op, _ctx| {
                op.set_n_pages(1);
            });

            self.print_operation.connect_draw_page(
                glib::clone!(@weak obj => move |_operation, _context, _page_nr| {
                        let imp = obj.imp();

                        imp.width.set_increments(1., 5.);
                        imp.margin_left.set_increments(1., 5.);
                        imp.margin_top.set_increments(1., 5.);

                obj.set_ranges();

                        imp.width.set_value(obj.image().image_size().0 as f64);
                        obj.center_horizontally();

                        obj.unit_selected();

                        imp.print_operation.cancel();

                        let orientation = match obj.page_setup().orientation() {
                            gtk::PageOrientation::Portrait => "portrait",
                            gtk::PageOrientation::Landscape => "landscape",
                            _ => "other",
                        };
                        obj.set_orientation(orientation);

                        obj.set_visible(true);
                    }),
            );
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }

    impl WidgetImpl for LpPrint {}
    impl WindowImpl for LpPrint {}
    impl AdwWindowImpl for LpPrint {}
}
glib::wrapper! {
    pub struct LpPrint(ObjectSubclass<imp::LpPrint>)
        @extends gtk::Widget, gtk::Window, adw::Window,
        @implements gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl LpPrint {
    pub fn new(
        image: LpImage,
        parent_window: gtk::Window,
        print_settings: Option<gtk::PrintSettings>,
        page_setup: Option<gtk::PageSetup>,
    ) -> Self {
        let print_settings = print_settings.unwrap_or_default();

        let obj: Self = glib::Object::builder()
            .property("image", image)
            .property("print-settings", print_settings)
            .property("parent-window", parent_window)
            .build();

        if let Some(page_setup) = page_setup {
            obj.print_operation()
                .set_default_page_setup(Some(&page_setup));
        }

        obj
    }

    pub fn unit(&self) -> glib::GString {
        self.imp()
            .unit
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|x| x.string())
            .unwrap_or_default()
    }

    pub fn dpi(&self) -> f64 {
        self.print_settings().resolution() as f64
    }

    #[template_callback]
    pub fn width_changed(&self) {
        self.imp()
            .scale
            .set_label(&format!("{:.2}\u{202F}%", self.user_scale() * 100.));

        self.draw_preview();
    }

    /// Returns user selected image width in pixels
    pub fn user_width(&self) -> f64 {
        f64::max(1., self.imp().width.value() / self.unit_factor())
    }

    /// Returns scaling of the original image based on selected image size
    pub fn user_scale(&self) -> f64 {
        let (orig_width, _) = self.image().image_size();

        self.user_width() / orig_width as f64
    }

    #[template_callback]
    pub fn margin_left_changed(&self) {
        self.draw_preview();
    }

    /// Returns user selected left margin in pixels
    pub fn user_margin_left(&self) -> f64 {
        self.imp().margin_left.value() / self.unit_factor()
    }

    #[template_callback]
    pub fn margin_top_changed(&self) {
        self.draw_preview();
    }

    /// Returns user selected top margin in pixels
    pub fn user_margin_top(&self) -> f64 {
        self.imp().margin_top.value() / self.unit_factor()
    }

    /// Redraws the preview widget child
    fn draw_preview(&self) {
        self.imp().preview.queue_resize();
    }

    /// Centers the image horizontally
    fn center_horizontally(&self) {
        let margin_left =
            (self.page_setup().paper_width(gtk::Unit::Inch) * self.dpi() - self.user_width()) / 2.;

        self.imp()
            .margin_left
            .set_value(margin_left * self.unit_factor());
    }

    /// Centers the image vertically
    fn center_vertically(&self) {
        let user_height = self.image().image_size().1 as f64 * self.user_scale();
        let margin_top =
            (self.page_setup().paper_height(gtk::Unit::Inch) * self.dpi() - user_height) / 2.;

        self.imp()
            .margin_top
            .set_value(margin_top * self.unit_factor());
    }

    /// Sets the allowed ranges for the spin button based on current unit
    fn set_ranges(&self) {
        let imp = self.imp();

        let max_height =
            self.page_setup().page_height(gtk::Unit::Inch) * self.dpi() * self.unit_factor();

        let max_width =
            self.page_setup().page_width(gtk::Unit::Inch) * self.dpi() * self.unit_factor();
        imp.width.set_range(self.unit_factor(), max_width);

        let min_left_margin =
            self.page_setup().left_margin(gtk::Unit::Inch) * self.dpi() * self.unit_factor();
        imp.margin_left
            .set_range(min_left_margin, min_left_margin + max_width);

        let min_top_margin =
            self.page_setup().top_margin(gtk::Unit::Inch) * self.dpi() * self.unit_factor();
        imp.margin_top
            .set_range(min_top_margin, min_top_margin + max_height);
    }

    #[template_callback]
    pub fn orientation_changed(&self) {
        let orientation = if self.orientation() == "landscape" {
            gtk::PageOrientation::Landscape
        } else {
            gtk::PageOrientation::Portrait
        };
        self.page_setup().set_orientation(orientation);
        self.set_ranges();
        self.draw_preview();
    }

    #[template_callback]
    pub fn unit_selected(&self) {
        let imp = self.imp();

        let (unit_factor, digits) = match self.unit().as_str() {
            "cm" => (2.54 / self.dpi(), 2),
            "in" => (1. / self.dpi(), 2),
            "px" => (1., 0),
            unit => {
                log::error!("Unknown unit '{unit}'");
                (1., 2)
            }
        };

        let update_factor = unit_factor / self.unit_factor();
        let width = imp.width.value() * update_factor;
        let margin_left = imp.margin_left.value() * update_factor;
        let margin_top = imp.margin_top.value() * update_factor;

        self.set_unit_factor(unit_factor);
        self.set_ranges();

        imp.width.set_digits(digits);
        imp.width.set_value(width);

        imp.margin_left.set_digits(digits);
        imp.margin_left.set_value(margin_left);

        imp.margin_top.set_digits(digits);
        imp.margin_top.set_value(margin_top);
    }

    pub fn run(&self) {
        self.print_operation()
            .run(
                gtk::PrintOperationAction::PrintDialog,
                self.imp().parent_window.get(),
            )
            .unwrap();
    }

    pub fn page_setup(&self) -> gtk::PageSetup {
        self.print_operation().default_page_setup()
    }

    /// Go back to print dialog from image layout
    async fn back(&self) {
        self.close();
        let print = Self::new(
            self.image(),
            self.parent_window(),
            self.print_operation().print_settings(),
            Some(self.print_operation().default_page_setup()),
        );
        print.run();
    }

    /// Initialize actual print operation
    fn print(&self) {
        self.close();

        let print_operation = gtk::PrintOperation::new();

        let print_settings = self.print_operation().print_settings();

        if let Some(print_settings) = &print_settings {
            if let Some(uri) = self.image().file().map(|x| x.uri()) {
                print_settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&format!("{uri}.pdf")));
            }
        }

        print_operation.set_print_settings(print_settings.as_ref());
        print_operation.set_default_page_setup(Some(&self.print_operation().default_page_setup()));

        print_operation.connect_begin_print(move |op, _ctx| {
            op.set_n_pages(1);
        });

        print_operation.connect_draw_page(
            glib::clone!(@weak self as obj => move |_operation, context, _page_nr| {
            obj.draw_page(context);
            }),
        );

        print_operation
            .run(
                gtk::PrintOperationAction::Print,
                Some(&self.parent_window()),
            )
            .unwrap();
    }

    /// Draw PDF for printing
    fn draw_page(&self, print_context: &gtk::PrintContext) {
        log::debug!("Drawing image to print");
        let image = self.image();

        let cairo_dpi = print_context.dpi_x();

        let (orig_width, orig_height) = image.image_size();

        let texture_scale = self.user_width() / orig_width as f64;
        let cairo_scale = cairo_dpi / self.dpi();

        let cairo_surface = if image.format().map_or(false, |x| x.is_svg()) {
            // Render SVG to exact needed sizes
            // TODO: This should be async
            decoder::formats::Svg::render_print(
                &image.file().unwrap(),
                (orig_width as f64 * texture_scale) as i32,
                (orig_height as f64 * texture_scale) as i32,
            )
            .unwrap()
        } else {
            let texture = image.print_data(texture_scale).unwrap();

            let mut downloader = gdk::TextureDownloader::new(&texture);
            downloader.set_format(gdk::MemoryFormat::B8g8r8a8Premultiplied);

            let (data, stride) = downloader.download_bytes();
            let data = data.to_vec();

            let width = texture.width();
            let height = texture.height();

            cairo::ImageSurface::create_for_data(
                data,
                cairo::Format::ARgb32,
                width,
                height,
                stride as i32,
            )
            .unwrap()
        };

        let cairo_context = print_context.cairo_context();
        cairo_context.scale(cairo_scale, cairo_scale);

        let margin_left =
            self.user_margin_left() - self.page_setup().left_margin(gtk::Unit::Inch) * self.dpi();
        let margin_top =
            self.user_margin_top() - self.page_setup().top_margin(gtk::Unit::Inch) * self.dpi();

        cairo_context
            .set_source_surface(cairo_surface, margin_left, margin_top)
            .unwrap();

        cairo_context.paint().unwrap();
    }
}
