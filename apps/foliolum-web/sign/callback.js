// TDPdf Sign's connect page sends the popup here with ?code=...&state=... (or ?error=...). Pass the
// query to the editor tab on a same-origin channel (src/sign_web.rs listens; it checks the state and
// trades the code itself), take the code out of the address bar, and close. A file, not inline, so the
// page's CSP (script-src 'self') allows it.
(function () {
  var query = window.location.search;
  try { window.history.replaceState(null, "", window.location.pathname); } catch (e) {}
  var note = document.getElementById("note");
  try {
    var channel = new BroadcastChannel("foliolum-sign-in");
    channel.postMessage(query);
    channel.close();
    note.textContent = "Signed in. You can close this window and go back to Foliolum.";
  } catch (e) {
    note.textContent = "This browser couldn't pass the sign-in back to Foliolum. Paste an API token instead.";
  }
  setTimeout(function () { window.close(); }, 300);
})();
