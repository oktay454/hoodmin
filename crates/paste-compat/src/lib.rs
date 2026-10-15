//! Leptos bileşenleri için bakımı süren pastey makrosuna uyumluluk yönlendirmesi.
//! Eski paste uygulamasını içermez; bütün makro işlemleri pastey tarafından yapılır.
pub use pastey::paste;

#[cfg(test)]
mod tests {
	use super::paste;
	paste! { const [<hello:upper _ world:upper>]: &str = "ok"; }
	#[test]
	fn concatenation_and_case_match() {
		assert_eq!(HELLO_WORLD, "ok");
	}
}
