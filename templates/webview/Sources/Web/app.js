(function () {
  'use strict';
  var taps = 0;
  var tapButton = document.getElementById('tap');

  function render() {
    tapButton.textContent = 'Tapped ' + taps + (taps === 1 ? ' time' : ' times');
  }

  native.device().then(function (d) {
    document.getElementById('device').textContent = d.name + ' · ' + d.system;
  });

  native.get('taps').then(function (v) {
    taps = parseInt(v, 10) || 0;
    render();
  });

  tapButton.addEventListener('click', function () {
    taps += 1;
    render();
    native.haptic();
    native.set('taps', taps);
  });

  document.getElementById('share').addEventListener('click', function () {
    native.share('I tapped ' + taps + ' times in __DISPLAY_NAME__!');
  });
})();
