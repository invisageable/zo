// zo template interactivity bridge
// Minimal runtime for event handling and state synchronization

class ZoRuntime {
  constructor() {
    this.ws = null;
    this.reconnectDelay = 1000;
    this.maxReconnectDelay = 10000;
    this.connected = false;
    this.setupEventListeners();
    this.tryConnect();
  }

  tryConnect() {
    // For static HTML preview, no WebSocket needed
    // This will be used later when we integrate with Codelord server
    console.log('[zo] Running in static mode');
  }

  connect() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      return;
    }

    try {
      this.ws = new WebSocket('ws://127.0.0.1:1337/zo/ws');

      this.ws.onopen = () => {
        console.log('[zo] Connected to runtime');
        this.connected = true;
        this.reconnectDelay = 1000;
      };

      this.ws.onclose = () => {
        console.log('[zo] Disconnected, retrying in', this.reconnectDelay, 'ms');
        this.connected = false;
        setTimeout(() => this.connect(), this.reconnectDelay);
        this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxReconnectDelay);
      };

      this.ws.onerror = (error) => {
        console.error('[zo] WebSocket error:', error);
      };

      this.ws.onmessage = (event) => {
        this.handleMessage(event);
      };
    } catch (error) {
      console.error('[zo] Connection failed:', error);
    }
  }

  handleMessage(event) {
    try {
      const msg = JSON.parse(event.data);

      switch (msg.type) {
        case 'update':
          this.updateElement(msg.id, msg.property, msg.value);
          break;
        case 'reload':
          window.location.reload();
          break;
        case 'error':
          this.showError(msg.error);
          break;
        default:
          console.warn('[zo] Unknown message type:', msg.type);
      }
    } catch (error) {
      console.error('[zo] Failed to handle message:', error);
    }
  }

  updateElement(id, property, value) {
    const el = document.querySelector(`[data-id="${id}"]`);
    if (el) {
      if (property === 'textContent') {
        el.textContent = value;
      } else if (property === 'value') {
        el.value = value;
      } else if (property === 'innerHTML') {
        el.innerHTML = value;
      }
    }
  }

  showError(error) {
    const overlay = document.createElement('div');
    overlay.style.cssText = `
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: rgba(0, 0, 0, 0.95);
      color: #ff6b6b;
      font-family: monospace;
      padding: 2rem;
      z-index: 999999;
      overflow: auto;
    `;
    overlay.innerHTML = `
      <h1 style="color: #ff6b6b; margin-bottom: 1rem;">⚠️ Compilation Error</h1>
      <pre style="white-space: pre-wrap; line-height: 1.5;">${this.escapeHtml(error)}</pre>
      <button onclick="this.parentElement.remove()" style="margin-top: 1rem; padding: 0.5rem 1rem; cursor: pointer;">
        Dismiss
      </button>
    `;
    document.body.appendChild(overlay);
  }

  setupEventListeners() {
    // Click events
    document.addEventListener('click', (e) => {
      const id = e.target.dataset.id;
      if (id && e.target.tagName === 'BUTTON') {
        this.sendEvent('click', id);
      }
    });

    // Input events
    document.addEventListener('input', (e) => {
      const id = e.target.dataset.id;
      if (id && e.target.tagName === 'INPUT') {
        this.sendEvent('input', id, e.target.value);
      }
    });
  }

  sendEvent(type, widgetId, data = null) {
    if (this.connected && this.ws && this.ws.readyState === WebSocket.OPEN) {
      const msg = {
        type: 'event',
        event_type: type,
        widget_id: parseInt(widgetId),
        data: data
      };
      this.ws.send(JSON.stringify(msg));
    } else {
      // In static mode, just log events
      console.log(`[zo] Event: ${type} on widget ${widgetId}`, data);
    }
  }

  escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    window.zoRuntime = new ZoRuntime();
  });
} else {
  window.zoRuntime = new ZoRuntime();
}

// Event handler functions (called from inline onclick/oninput)
function handleClick(id) {
  if (window.zoRuntime) {
    window.zoRuntime.sendEvent('click', id);
  }
}

function handleInput(id, value) {
  if (window.zoRuntime) {
    window.zoRuntime.sendEvent('input', id, value);
  }
}
