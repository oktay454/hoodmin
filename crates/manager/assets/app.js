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
	select.form.classList.add("enhanced");
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

// Tema düğmesi form verilerini korur; JavaScript kapalıyken standart form çalışır.
document.querySelectorAll(".theme-switch").forEach(form => {
	const button = form.querySelector(".theme-toggle");
	let saving = false;
	form.addEventListener("submit", async event => {
		event.preventDefault();
		if (saving) return;
		saving = true;
		const requested = form.elements.theme.value;
		const hadFocus = document.activeElement === button;
		const feedback = form.querySelector(".theme-feedback");
		feedback.textContent = "";
		button.disabled = true;
		try {
			const query = new URLSearchParams({theme: requested, return_to: form.elements.return_to.value});
			const response = await fetch(form.action + "?" + query, {headers: {Accept: "application/json"}, redirect: "error", cache: "no-store"});
			if (!response.ok) throw new Error("Theme preference failed");
			document.documentElement.dataset.theme = requested;
			document.querySelector('meta[name="color-scheme"]').content = requested;
			form.elements.theme.value = requested === "dark" ? "light" : "dark";
			const label = requested === "dark" ? document.documentElement.dataset.themeLight : document.documentElement.dataset.themeDark;
			button.title = label;
			button.setAttribute("aria-label", label);
		} catch {
			feedback.textContent = document.documentElement.dataset.themeError;
		} finally {
			button.disabled = false;
			saving = false;
			if (hadFocus) button.focus({preventScroll: true});
		}
	});
});

// PFX remains in this page and is posted only to the same-origin form action.
document.querySelectorAll("[data-pfx-file]").forEach(input => {
	input.addEventListener("change", () => {
		const target = document.getElementById(input.dataset.pfxFile);
		target.value = "";
		const file = input.files[0];
		if (!file || file.size > 256 * 1024) return;
		const reader = new FileReader();
		reader.addEventListener("load", () => { target.value = reader.result.split(",",2)[1]; });
		reader.readAsDataURL(file);
	});
});

// Refresh job results without clearing an unfinished SSH form.
document.querySelectorAll("[data-install-job]").forEach(section => {
	let stopped = false;
	const poll = async () => {
		if (stopped) return;
		try {
			const response = await fetch(section.dataset.installJob, {cache: "no-store"});
			if (!response.ok) return;
			const job = await response.json();
			section.querySelector("[data-install-status]").textContent = job.label || job.status;
			section.querySelector("[data-install-log]").textContent = job.log;
			if (["Succeeded", "Failed"].includes(job.status)) return;
		} catch { return; }
		setTimeout(poll, 3000);
	};
	poll();
	window.addEventListener("pagehide", () => { stopped = true; }, {once:true});
});
