(function() {
  // Click events on buttons with `data-id`. Text children of a button are
  // wrapped in `<span data-zo-cmd="...">` nodes, so `event.target` is usually
  // the span, not the button itself — walk up via closest() to resolve the 
  // nearest button.
  document.addEventListener('click', function(event) {
    const button = event.target.closest && event.target.closest('button');

    if (button) {
      const id = button.dataset.id;

      if (id && window.ipc) {
        window.ipc.postMessage('click:' + id);
      }
    }
  });

  // Input events on inputs with data-id.
  document.addEventListener('input', function(event) {
    const input = event.target.closest
      && event.target.closest('input, textarea');

    if (input) {
      const id = input.dataset.id;

      if (id && window.ipc) {
        window.ipc.postMessage('input:' + id + ':' + input.value);
      }
    }
  });

  // Focus events on elements with data-id.
  document.addEventListener('focus', function(event) {
    const el = event.target.closest && event.target.closest('[data-id]');

    if (el) {
      const id = el.dataset.id;

      if (id && window.ipc) {
        window.ipc.postMessage('focus:' + id);
      }
    }
  }, true);

  // Keyboard interactions on text inputs. We dispatch our own kind frame
  // instead of the native `submit` DOM event since zo inputs aren't wrapped
  // in a <form>. preventDefault fires only when a key is claimed so unrelated
  // keys pass through.
  document.addEventListener('keydown', function(event) {
    if (event.key !== 'Enter') {
      return;
    }

    const input = event.target.closest
      && event.target.closest('input, textarea');

    if (input) {
      const id = input.dataset.id;

      if (id && window.ipc) {
        event.preventDefault();
        window.ipc.postMessage('submit:' + id + ':' + input.value);
      }
    }
  });
})();
