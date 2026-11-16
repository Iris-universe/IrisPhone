// Bridge to the Swift shell (WebAppView.swift). In a desktop browser, a fake implementation
// is used instead so the web app can be developed and tested without an iPhone.
(function () {
  'use strict';
  var handler = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.native;

  function call(op, args) {
    var msg = Object.assign({ op: op }, args || {});
    if (handler) return handler.postMessage(msg);
    return Promise.resolve(fake(msg));
  }

  function fake(msg) {
    switch (msg.op) {
      case 'device': return { name: 'Desktop browser', model: navigator.platform, system: 'web', native: false };
      case 'haptic': return true;
      case 'storeGet': return localStorage.getItem('web.' + msg.key);
      case 'storeSet': localStorage.setItem('web.' + msg.key, msg.value); return true;
      case 'share': console.log('[share]', msg.text); return true;
      default: return Promise.reject(new Error('unknown op: ' + msg.op));
    }
  }

  window.native = {
    isNative: !!handler,
    call: call,
    device: function () { return call('device'); },
    haptic: function () { return call('haptic'); },
    get: function (key) { return call('storeGet', { key: key }); },
    set: function (key, value) { return call('storeSet', { key: key, value: String(value) }); },
    share: function (text) { return call('share', { text: String(text) }); }
  };
})();
