"use strict";
setInterval(async () => {
	try {
		const response = await fetch("/api/status", { cache: "no-store" });
		if (response.ok && (await response.json()).ready) location.replace("/panel");
	} catch { /* Keep the simple unavailable page while disconnected. */ }
}, 5000);
