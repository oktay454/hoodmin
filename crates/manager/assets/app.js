"use strict";

const selected = () => Array.from(document.querySelectorAll(".token-select:checked"))
	.map(input => input.value);

const syncSelection = () => {
	const ids = selected();
	const target = document.getElementById("selected-ids");
	if (target) target.value = ids.join(" ");
	const count = document.getElementById("selected-count");
	if (count) count.textContent = String(ids.length);
};

document.querySelectorAll(".token-select").forEach(input => {
	input.addEventListener("change", syncSelection);
});

document.getElementById("select-all")?.addEventListener("change", event => {
	document.querySelectorAll(".token-select").forEach(input => {
		input.checked = event.target.checked;
	});
	syncSelection();
});

document.getElementById("bulk-form")?.addEventListener("submit", event => {
	syncSelection();
	if (selected().length === 0) {
		event.preventDefault();
		document.getElementById("selected-count").focus();
	}
});

document.querySelectorAll("[data-copy]").forEach(button => {
	button.addEventListener("click", async () => {
		const source = document.getElementById(button.dataset.copy);
		const feedback = document.getElementById("copy-feedback");
		try {
			await navigator.clipboard.writeText(source.value);
			feedback.textContent = "Kopyalandı.";
		} catch {
			source.focus();
			source.select();
			feedback.textContent = "Metin seçildi. Kopyalama kısayolunu kullanabilirsiniz.";
		}
	});
});

if (document.querySelector("[data-stat]")) {
	const source = new EventSource("/events");
	source.addEventListener("stats", event => {
		try {
			const values = JSON.parse(event.data);
			document.querySelectorAll("[data-stat]").forEach(node => {
				if (Object.hasOwn(values, node.dataset.stat)) {
					node.textContent = String(values[node.dataset.stat]);
				}
			});
		} catch {
			source.close();
		}
	});
	window.addEventListener("pagehide", () => source.close(), {once: true});
}
