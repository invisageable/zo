// zo template interactivity bridge
// Routes DOM events to the native runtime via wry IPC.

(function() {
  // Click events on buttons with data-id.
  document.addEventListener('click', function(e) {
    var id = e.target.dataset.id;

    if (id && e.target.tagName === 'BUTTON') {
      if (window.ipc) {
        window.ipc.postMessage('click:' + id);
      }
    }
  });

  // Input events on inputs with data-id.
  document.addEventListener('input', function(e) {
    var id = e.target.dataset.id;

    if (id && e.target.tagName === 'INPUT') {
      if (window.ipc) {
        window.ipc.postMessage('input:' + id + ':' + e.target.value);
      }
    }
  });

  // Focus events on elements with data-id.
  document.addEventListener('focus', function(e) {
    var id = e.target.dataset.id;

    if (id && window.ipc) {
      window.ipc.postMessage('focus:' + id);
    }
  }, true);
})();
