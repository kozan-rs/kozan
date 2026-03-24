//! Type-safe style API — zero CSS parsing, direct `PropertyDeclaration`.
//!
//! Fluent builder with batch flush:
//! ```ignore
//! div.style()
//!     .width(px(200.0))
//!     .height(pct(100.0))
//!     .background_color(rgb(0.9, 0.3, 0.2))
//!     .padding(px(16.0));
//! // ^ all declarations flushed in ONE write on drop
//! ```

use style::color::AbsoluteColor;
use style::properties::PropertyDeclaration;
use style::values::specified::box_::{Display, PositionProperty};
use style::values::specified::{
    Color, FontSize, FontWeight, Margin, NonNegativeLengthPercentage, Opacity, Size,
};

use crate::dom::document_cell::DocumentCell;
use crate::id::RawId;

/// Batched style builder — collects declarations, flushes in ONE write on drop.
///
/// Like JavaScript's `element.style` but batched: setting N properties
/// costs one lock acquisition, not N. Chrome does the same — style mutations
/// are batched within a microtask.
pub struct StyleAccess {
    cell: DocumentCell,
    id: RawId,
    pending: Vec<PropertyDeclaration>,
}

impl StyleAccess {
    pub(crate) fn new(cell: DocumentCell, id: RawId) -> Self {
        Self {
            cell,
            id,
            pending: Vec::new(),
        }
    }

    // ── Display ──

    pub fn display(&mut self, value: Display) -> &mut Self {
        self.pending.push(PropertyDeclaration::Display(value));
        self
    }

    // ── Color ──

    pub fn color(&mut self, value: impl Into<AbsoluteColor>) -> &mut Self {
        use style::values::specified::color::ColorPropertyValue;
        self.pending
            .push(PropertyDeclaration::Color(ColorPropertyValue(
                Color::from_absolute_color(value.into()),
            )));
        self
    }

    pub fn background_color(&mut self, value: impl Into<AbsoluteColor>) -> &mut Self {
        self.pending.push(PropertyDeclaration::BackgroundColor(
            Color::from_absolute_color(value.into()),
        ));
        self
    }

    // ── Dimensions ──

    pub fn width(&mut self, value: impl Into<Size>) -> &mut Self {
        self.pending.push(PropertyDeclaration::Width(value.into()));
        self
    }

    pub fn height(&mut self, value: impl Into<Size>) -> &mut Self {
        self.pending.push(PropertyDeclaration::Height(value.into()));
        self
    }

    pub fn min_width(&mut self, value: impl Into<Size>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::MinWidth(value.into()));
        self
    }

    pub fn min_height(&mut self, value: impl Into<Size>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::MinHeight(value.into()));
        self
    }

    // ── Margin ──

    pub fn margin_top(&mut self, value: impl Into<Margin>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::MarginTop(value.into()));
        self
    }

    pub fn margin_right(&mut self, value: impl Into<Margin>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::MarginRight(value.into()));
        self
    }

    pub fn margin_bottom(&mut self, value: impl Into<Margin>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::MarginBottom(value.into()));
        self
    }

    pub fn margin_left(&mut self, value: impl Into<Margin>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::MarginLeft(value.into()));
        self
    }

    pub fn margin(&mut self, value: impl Into<Margin>) -> &mut Self {
        let v = value.into();
        self.pending.push(PropertyDeclaration::MarginTop(v.clone()));
        self.pending
            .push(PropertyDeclaration::MarginRight(v.clone()));
        self.pending
            .push(PropertyDeclaration::MarginBottom(v.clone()));
        self.pending.push(PropertyDeclaration::MarginLeft(v));
        self
    }

    // ── Padding ──

    pub fn padding_top(&mut self, value: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::PaddingTop(value.into()));
        self
    }

    pub fn padding_right(&mut self, value: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::PaddingRight(value.into()));
        self
    }

    pub fn padding_bottom(&mut self, value: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::PaddingBottom(value.into()));
        self
    }

    pub fn padding_left(&mut self, value: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        self.pending
            .push(PropertyDeclaration::PaddingLeft(value.into()));
        self
    }

    pub fn padding(&mut self, value: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        let v = value.into();
        self.pending
            .push(PropertyDeclaration::PaddingTop(v.clone()));
        self.pending
            .push(PropertyDeclaration::PaddingRight(v.clone()));
        self.pending
            .push(PropertyDeclaration::PaddingBottom(v.clone()));
        self.pending.push(PropertyDeclaration::PaddingLeft(v));
        self
    }

    // ── Position ──

    pub fn position(&mut self, value: PositionProperty) -> &mut Self {
        self.pending.push(PropertyDeclaration::Position(value));
        self
    }

    // ── Font ──

    pub fn font_size(&mut self, value: FontSize) -> &mut Self {
        self.pending.push(PropertyDeclaration::FontSize(value));
        self
    }

    pub fn font_weight(&mut self, value: FontWeight) -> &mut Self {
        self.pending.push(PropertyDeclaration::FontWeight(value));
        self
    }

    /// Set font-family from a name string (e.g., "Cairo", "Roboto").
    ///
    /// Chrome equivalent: setting `element.style.fontFamily`.
    /// Constructs a Stylo `FontFamily` with a single named family.
    ///
    /// For multiple families or generics, use `set_attribute("style", "font-family: ...")`.
    pub fn font_family(&mut self, name: &str) -> &mut Self {
        use style::Atom;
        use style::values::computed::font::{
            FamilyName, FontFamilyList, FontFamilyNameSyntax, SingleFontFamily,
        };
        use style::values::specified::font::FontFamily;

        let list = FontFamilyList {
            list: style::ArcSlice::from_iter(std::iter::once(SingleFontFamily::FamilyName(
                FamilyName {
                    name: Atom::from(name),
                    syntax: FontFamilyNameSyntax::Quoted,
                },
            ))),
        };
        self.pending
            .push(PropertyDeclaration::FontFamily(FontFamily::Values(list)));
        self
    }

    // ── Opacity ──

    pub fn opacity(&mut self, value: Opacity) -> &mut Self {
        self.pending.push(PropertyDeclaration::Opacity(value));
        self
    }

    // ── Flex ──

    pub fn flex_grow(&mut self, value: f32) -> &mut Self {
        use style::values::generics::NonNegative;
        use style::values::specified::Number;
        self.pending
            .push(PropertyDeclaration::FlexGrow(NonNegative(Number::new(
                value,
            ))));
        self
    }

    pub fn flex_shrink(&mut self, value: f32) -> &mut Self {
        use style::values::generics::NonNegative;
        use style::values::specified::Number;
        self.pending
            .push(PropertyDeclaration::FlexShrink(NonNegative(Number::new(
                value,
            ))));
        self
    }

    pub fn flex_basis(&mut self, value: impl Into<Size>) -> &mut Self {
        use style::values::generics::flex::GenericFlexBasis;
        use style::values::generics::length::GenericSize;
        let size = value.into();
        let basis = match size {
            GenericSize::Auto => GenericFlexBasis::Content,
            other => GenericFlexBasis::Size(other),
        };
        self.pending
            .push(PropertyDeclaration::FlexBasis(Box::new(basis)));
        self
    }

    // ── Gap ──

    pub fn gap(&mut self, value: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        use style::values::generics::length::GenericLengthPercentageOrNormal;
        let v: NonNegativeLengthPercentage = value.into();
        self.pending.push(PropertyDeclaration::RowGap(
            GenericLengthPercentageOrNormal::LengthPercentage(v.clone()),
        ));
        self.pending.push(PropertyDeclaration::ColumnGap(
            GenericLengthPercentageOrNormal::LengthPercentage(v),
        ));
        self
    }

    // ── Flex layout ──

    pub fn flex_direction(
        &mut self,
        value: style::computed_values::flex_direction::T,
    ) -> &mut Self {
        self.pending.push(PropertyDeclaration::FlexDirection(value));
        self
    }

    pub fn flex_wrap(&mut self, value: style::computed_values::flex_wrap::T) -> &mut Self {
        self.pending.push(PropertyDeclaration::FlexWrap(value));
        self
    }

    /// `display: flex; flex-direction: row`
    pub fn flex_row(&mut self) -> &mut Self {
        self.display(Display::Flex);
        self.flex_direction(style::computed_values::flex_direction::T::Row)
    }

    /// `display: flex; flex-direction: column`
    pub fn flex_col(&mut self) -> &mut Self {
        self.display(Display::Flex);
        self.flex_direction(style::computed_values::flex_direction::T::Column)
    }

    // ── Alignment ──

    pub fn align_items_center(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ItemPlacement};
        self.pending
            .push(PropertyDeclaration::AlignItems(ItemPlacement(
                AlignFlags::CENTER,
            )));
        self
    }
    pub fn align_items_start(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ItemPlacement};
        self.pending
            .push(PropertyDeclaration::AlignItems(ItemPlacement(
                AlignFlags::FLEX_START,
            )));
        self
    }
    pub fn align_items_end(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ItemPlacement};
        self.pending
            .push(PropertyDeclaration::AlignItems(ItemPlacement(
                AlignFlags::FLEX_END,
            )));
        self
    }
    pub fn align_items_stretch(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ItemPlacement};
        self.pending
            .push(PropertyDeclaration::AlignItems(ItemPlacement(
                AlignFlags::STRETCH,
            )));
        self
    }

    pub fn justify_content_center(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ContentDistribution};
        self.pending.push(PropertyDeclaration::JustifyContent(
            ContentDistribution::new(AlignFlags::CENTER),
        ));
        self
    }
    pub fn justify_content_start(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ContentDistribution};
        self.pending.push(PropertyDeclaration::JustifyContent(
            ContentDistribution::new(AlignFlags::FLEX_START),
        ));
        self
    }
    pub fn justify_content_end(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ContentDistribution};
        self.pending.push(PropertyDeclaration::JustifyContent(
            ContentDistribution::new(AlignFlags::FLEX_END),
        ));
        self
    }
    pub fn justify_content_between(&mut self) -> &mut Self {
        use style::values::specified::align::{AlignFlags, ContentDistribution};
        self.pending.push(PropertyDeclaration::JustifyContent(
            ContentDistribution::new(AlignFlags::SPACE_BETWEEN),
        ));
        self
    }

    // ── Border ──

    pub fn border_radius(
        &mut self,
        v: impl Into<NonNegativeLengthPercentage> + Clone,
    ) -> &mut Self {
        use style::values::generics::border::GenericBorderCornerRadius;
        use style::values::generics::size::Size2D;
        let lp = v.into();
        let r = GenericBorderCornerRadius(Size2D::new(lp.clone(), lp));
        self.pending
            .push(PropertyDeclaration::BorderTopLeftRadius(Box::new(
                r.clone(),
            )));
        self.pending
            .push(PropertyDeclaration::BorderTopRightRadius(Box::new(
                r.clone(),
            )));
        self.pending
            .push(PropertyDeclaration::BorderBottomRightRadius(Box::new(
                r.clone(),
            )));
        self.pending
            .push(PropertyDeclaration::BorderBottomLeftRadius(Box::new(r)));
        self
    }

    pub fn border_width(&mut self, v: impl Into<NonNegativeLengthPercentage> + Clone) -> &mut Self {
        // border-width uses NonNegativeLength (from Au), but we can use the medium keyword
        // fallback. For exact px, use set_attribute("style", "border-width: Npx").
        // Here we use a CSS string approach internally.
        let lp = v.into();
        // Extract px value from the NonNegativeLengthPercentage
        self.pending.push(PropertyDeclaration::BorderTopWidth(
            style::values::specified::BorderSideWidth::from_px(self.extract_px(&lp)),
        ));
        self.pending.push(PropertyDeclaration::BorderRightWidth(
            style::values::specified::BorderSideWidth::from_px(self.extract_px(&lp)),
        ));
        self.pending.push(PropertyDeclaration::BorderBottomWidth(
            style::values::specified::BorderSideWidth::from_px(self.extract_px(&lp)),
        ));
        self.pending.push(PropertyDeclaration::BorderLeftWidth(
            style::values::specified::BorderSideWidth::from_px(self.extract_px(&lp)),
        ));
        self
    }

    pub fn border_style(&mut self, v: style::values::specified::border::BorderStyle) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderTopStyle(v));
        self.pending.push(PropertyDeclaration::BorderRightStyle(v));
        self.pending.push(PropertyDeclaration::BorderBottomStyle(v));
        self.pending.push(PropertyDeclaration::BorderLeftStyle(v));
        self
    }

    pub fn border_color(&mut self, v: impl Into<AbsoluteColor>) -> &mut Self {
        let c = Color::from_absolute_color(v.into());
        self.pending
            .push(PropertyDeclaration::BorderTopColor(c.clone()));
        self.pending
            .push(PropertyDeclaration::BorderRightColor(c.clone()));
        self.pending
            .push(PropertyDeclaration::BorderBottomColor(c.clone()));
        self.pending.push(PropertyDeclaration::BorderLeftColor(c));
        self
    }

    pub fn border_bottom_width_px(&mut self, v: f32) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderBottomWidth(
            style::values::specified::BorderSideWidth::from_px(v),
        ));
        self
    }

    pub fn border_bottom_style(
        &mut self,
        v: style::values::specified::border::BorderStyle,
    ) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderBottomStyle(v));
        self
    }

    pub fn border_bottom_color(&mut self, v: impl Into<AbsoluteColor>) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderBottomColor(
            Color::from_absolute_color(v.into()),
        ));
        self
    }

    pub fn border_right_width_px(&mut self, v: f32) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderRightWidth(
            style::values::specified::BorderSideWidth::from_px(v),
        ));
        self
    }

    pub fn border_right_style(
        &mut self,
        v: style::values::specified::border::BorderStyle,
    ) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderRightStyle(v));
        self
    }

    pub fn border_right_color(&mut self, v: impl Into<AbsoluteColor>) -> &mut Self {
        self.pending.push(PropertyDeclaration::BorderRightColor(
            Color::from_absolute_color(v.into()),
        ));
        self
    }

    // ── Overflow ──

    pub fn overflow_hidden(&mut self) -> &mut Self {
        use style::values::specified::box_::Overflow;
        self.pending
            .push(PropertyDeclaration::OverflowX(Overflow::Hidden));
        self.pending
            .push(PropertyDeclaration::OverflowY(Overflow::Hidden));
        self
    }

    // ── Internal helper ──

    #[inline]
    fn extract_px(&self, lp: &NonNegativeLengthPercentage) -> f32 {
        use style::values::specified::length::NoCalcLength;
        match &lp.0 {
            style::values::specified::LengthPercentage::Length(NoCalcLength::Absolute(
                style::values::specified::length::AbsoluteLength::Px(v),
            )) => *v,
            _ => 0.0,
        }
    }

    // ── Short aliases (GPUI-style) ──
    // One letter or short name, same batched flush.
    //
    // container.style().w(px(200.0)).h(px(100.0)).bg(rgb8(232, 76, 61));

    /// Short for `width`.
    pub fn w(&mut self, v: impl Into<Size>) -> &mut Self {
        self.width(v)
    }
    /// Short for `height`.
    pub fn h(&mut self, v: impl Into<Size>) -> &mut Self {
        self.height(v)
    }
    /// Width + height (square).
    pub fn size(&mut self, v: impl Into<Size> + Clone) -> &mut Self {
        self.width(v.clone());
        self.height(v)
    }
    /// Short for `background_color`.
    pub fn bg(&mut self, v: impl Into<AbsoluteColor>) -> &mut Self {
        self.background_color(v)
    }
    /// `display: flex`.
    pub fn flex(&mut self) -> &mut Self {
        self.display(Display::Flex)
    }
    /// `display: grid`.
    pub fn grid(&mut self) -> &mut Self {
        self.display(Display::Grid)
    }
    /// `display: block`.
    pub fn block(&mut self) -> &mut Self {
        self.display(Display::Block)
    }
    /// Short for `padding` (all 4).
    pub fn pad(&mut self, v: impl Into<NonNegativeLengthPercentage>) -> &mut Self {
        self.padding(v)
    }
    /// Short for `margin` (all 4).
    pub fn mar(&mut self, v: impl Into<Margin>) -> &mut Self {
        self.margin(v)
    }

    // ── Raw (advanced) ──

    pub fn raw(&mut self, decl: PropertyDeclaration) -> &mut Self {
        self.pending.push(decl);
        self
    }

    // ── Explicit flush ──

    /// Flush all pending declarations NOW.
    /// Not needed for chains — Drop flushes automatically at semicolon.
    pub fn apply(&mut self) {
        self.flush();
    }

    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let index = self.id.index();
        self.cell.write(|doc| {
            doc.write_element_data(self.id, |ed| {
                for decl in pending {
                    ed.set_inline_property(decl);
                }
            });
            // Mark element + propagate dirty_descendants up ancestors.
            doc.mark_for_restyle(index);
        });
    }
}

/// Auto-flush on drop — chains flush at the semicolon.
impl Drop for StyleAccess {
    fn drop(&mut self) {
        self.flush();
    }
}
