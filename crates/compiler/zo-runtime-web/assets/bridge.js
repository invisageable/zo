// zo template interactivity bridge
// Routes DOM events to the native runtime via wry IPC.

(function() {
  // Click events on buttons with data-id. Text children of a
  // button are wrapped in <span data-zo-cmd="..."> nodes, so
  // `e.target` is usually the span, not the button itself —
  // walk up via closest() to resolve the nearest button.
  document.addEventListener('click', function(e) {
    var button = e.target.closest && e.target.closest('button');

    if (button) {
      var id = button.dataset.id;

      if (id && window.ipc) {
        window.ipc.postMessage('click:' + id);
      }
    }
  });

  // Input events on inputs with data-id.
  document.addEventListener('input', function(e) {
    var input = e.target.closest && e.target.closest('input, textarea');

    if (input) {
      var id = input.dataset.id;

      if (id && window.ipc) {
        window.ipc.postMessage('input:' + id + ':' + input.value);
      }
    }
  });

  // Focus events on elements with data-id.
  document.addEventListener('focus', function(e) {
    var el = e.target.closest && e.target.closest('[data-id]');

    if (el) {
      var id = el.dataset.id;

      if (id && window.ipc) {
        window.ipc.postMessage('focus:' + id);
      }
    }
  }, true);
})();
