// A sign-in popup lands here with ?code=...&state=... (or ?error=...): TDPdf Sign's connect page at
// /sign/callback, Claviger's at /auth/callback. Pass the query to the editor tab on a same-origin
// channel (src/sign_web.rs or src/account_web.rs listens; it checks the state and trades the code
// itself), take the code out of the address bar, and close. A file, not inline, so the
// page's CSP (script-src 'self') allows it.
(function () {
  var query = window.location.search;
  try { window.history.replaceState(null, "", window.location.pathname); } catch (e) {}
  var note = document.getElementById("note");
  try {
    // The page says which channel: foliolum-sign-in (TDPdf Sign, /sign/callback) or foliolum-auth
    // (Claviger, /auth/callback).
    var name = (document.currentScript && document.currentScript.dataset.channel) || "foliolum-sign-in";
    var channel = new BroadcastChannel(name);
    channel.postMessage(query);
    channel.close();
    note.textContent = "Signed in. You can close this window and go back to Foliolum.";
  } catch (e) {
    note.textContent = "This browser couldn't pass the sign-in back to Foliolum. Try again, or use another browser.";
  }
  setTimeout(function () { window.close(); }, 300);
})();
