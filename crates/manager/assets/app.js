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
			feedback.textContent = document.documentElement.dataset.copySuccess;
		} catch {
			source.focus();
			source.select();
			feedback.textContent = document.documentElement.dataset.copyFallback;
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

// Dil değişimi güvenli, aynı kaynak üzerindeki tercih uç noktasına gider.
document.querySelectorAll(".language-switch select").forEach(select => {
	select.addEventListener("change", () => select.form.requestSubmit());
});

const helpDialog = document.getElementById("help-dialog");
let helpTrigger;
document.querySelectorAll("[data-help-title]").forEach(button => {
	button.addEventListener("click", event => {
		event.preventDefault();
		helpTrigger = button;
		document.getElementById("help-title").textContent = button.dataset.helpTitle;
		document.getElementById("help-body").textContent = button.dataset.helpBody;
		helpDialog.showModal();
	});
});
helpDialog?.addEventListener("close", () => helpTrigger?.focus());

const confirmDialog = document.getElementById("confirm-dialog");
const confirmedForms = new WeakSet();
let pendingForm;
let confirmTrigger;
document.querySelectorAll("form[data-confirm]").forEach(form => {
	form.addEventListener("submit", event => {
		if (event.defaultPrevented) return;
		if (confirmedForms.delete(form)) return;
		event.preventDefault();
		pendingForm = form;
		confirmTrigger = document.activeElement;
		document.getElementById("confirm-body").textContent = form.dataset.confirm;
		confirmDialog.showModal();
	});
});
document.getElementById("confirm-cancel")?.addEventListener("click", () => confirmDialog.close());
document.getElementById("confirm-proceed")?.addEventListener("click", () => {
	const form = pendingForm;
	if (!form) return;
	pendingForm = null;
	confirmedForms.add(form);
	confirmDialog.close();
	form.requestSubmit();
});
confirmDialog?.addEventListener("close", () => {
	pendingForm = null;
	confirmTrigger?.focus();
});

[helpDialog, confirmDialog].forEach(dialog => {
	dialog?.addEventListener("click", event => {
		const bounds = dialog.getBoundingClientRect();
		if (event.target === dialog && (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom)) {
			dialog.close();
		}
	});
});
