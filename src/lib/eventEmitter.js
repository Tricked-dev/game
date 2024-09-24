//@ts-nocheck

/**
 * @template T
 * @param {T[]} haystack
 * @param {T} needle
 * @returns number
 */
function indexOf(haystack, needle) {
  return haystack.indexOf(needle);
}

/**
 * @class EventEmitter
 * @property {Record<string, (...args:unknown[])=>unknown>} events
 */
var EventEmitter = function () {
  this.events = {};
};

/**
 * @param {string} event
 * @param {function} listener
 */
EventEmitter.prototype.on = function (event, listener) {
  if (typeof this.events[event] !== "object") {
    this.events[event] = [];
  }

  this.events[event].push(listener);
};

EventEmitter.prototype.removeListener = function (event, listener) {
  var idx;

  if (typeof this.events[event] === "object") {
    idx = indexOf(this.events[event], listener);

    if (idx > -1) {
      this.events[event].splice(idx, 1);
    }
  }
};

/**
 * @param {string} event
 * @param {unknown[]} params
 */
EventEmitter.prototype.emit = function (event) {
  var i,
    listeners,
    length,
    args = [].slice.call(arguments, 1);

  if (typeof this.events[event] === "object") {
    listeners = this.events[event].slice();
    length = listeners.length;

    for (i = 0; i < length; i++) {
      listeners[i].apply(this, args);
    }
  }
};

/**
 * @param {string} event
 * @param {function} listener
 */
EventEmitter.prototype.once = function (event, listener) {
  this.on(event, function g() {
    this.removeListener(event, g);
    listener.apply(this, arguments);
  });
};

export { EventEmitter };
