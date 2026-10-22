// Galerie web d'Afterdusk : visionneuse et sélection client, sans serveur.
// La sélection reste dans le navigateur du client jusqu'à ce qu'il l'envoie
// par courriel ou en fichier ; rien ne part ailleurs.
(function () {
  "use strict";

  var data = JSON.parse(document.getElementById("gallery-data").textContent);
  var t = data.texts;
  var key = "afterdusk-selection-" + data.code;
  var picked = load();

  function load() {
    try {
      var raw = window.localStorage.getItem(key);
      var list = raw ? JSON.parse(raw) : [];
      return new Set(list.filter(Number.isInteger));
    } catch (e) {
      return new Set();
    }
  }

  function store() {
    try {
      window.localStorage.setItem(key, JSON.stringify(sorted()));
    } catch (e) {
      // Sans stockage, la sélection vaut pour la visite.
    }
  }

  function sorted() {
    return Array.from(picked).sort(function (a, b) {
      return a - b;
    });
  }

  function fill(text, values) {
    return text.replace(/\{(\w+)\}/g, function (m, name) {
      return name in values ? values[name] : m;
    });
  }

  function photoLabel(n) {
    return fill(t.photo, { n: n });
  }

  /** Texte renvoyé au photographe ; la ligne entre crochets est relue par Afterdusk. */
  function selectionText() {
    var list = sorted();
    return [
      data.title,
      fill(t.listIntro, { count: list.length }) + " " + list.join(", "),
      "",
      "[" + data.code + ": " + list.join(" ") + "]",
    ].join("\n");
  }

  var tiles = Array.prototype.slice.call(document.querySelectorAll(".tile"));
  var bar = document.querySelector(".bar");
  var count = document.querySelector(".bar .count");
  var sendButton = document.querySelector(".bar .open-send");
  var send = document.querySelector(".send");

  function refresh() {
    tiles.forEach(function (tile) {
      var n = Number(tile.dataset.n);
      var button = tile.querySelector(".pick");
      if (button) {
        button.setAttribute("aria-pressed", String(picked.has(n)));
        button.setAttribute("aria-label", fill(picked.has(n) ? t.unpick : t.pick, { n: n }));
      }
    });
    if (count) {
      count.textContent =
        picked.size === 0
          ? t.emptySelection
          : fill(picked.size === 1 ? t.selectionOne : t.selectionOther, { count: picked.size });
    }
    if (sendButton) sendButton.disabled = picked.size === 0;
    if (picked.size === 0 && send) send.hidden = true;
    syncLightbox();
  }

  function toggle(n) {
    if (picked.has(n)) picked.delete(n);
    else picked.add(n);
    store();
    refresh();
  }

  tiles.forEach(function (tile) {
    var n = Number(tile.dataset.n);
    var pick = tile.querySelector(".pick");
    if (pick) {
      pick.addEventListener("click", function () {
        toggle(n);
      });
    }
    tile.querySelector(".open").addEventListener("click", function () {
      open(tiles.indexOf(tile));
    });
  });

  // Envoi de la sélection : courriel préparé, fichier ou copie.
  if (bar && data.selection) {
    sendButton.addEventListener("click", function () {
      send.hidden = !send.hidden;
    });
    var mail = send.querySelector(".mail");
    if (mail) {
      mail.addEventListener("click", function () {
        var subject = fill(t.mailSubject, { title: data.title });
        mail.href =
          "mailto:" +
          encodeURIComponent(data.email) +
          "?subject=" +
          encodeURIComponent(subject) +
          "&body=" +
          encodeURIComponent(selectionText());
      });
    }
    send.querySelector(".file").addEventListener("click", function () {
      var blob = new Blob([selectionText() + "\n"], { type: "text/plain;charset=utf-8" });
      var link = document.createElement("a");
      link.href = URL.createObjectURL(blob);
      link.download = "selection-" + data.code + ".txt";
      document.body.appendChild(link);
      link.click();
      link.remove();
      setTimeout(function () {
        URL.revokeObjectURL(link.href);
      }, 1000);
    });
    var copy = send.querySelector(".copy");
    copy.addEventListener("click", function () {
      var done = function () {
        copy.textContent = t.copied;
        setTimeout(function () {
          copy.textContent = t.copy;
        }, 2000);
      };
      if (navigator.clipboard) navigator.clipboard.writeText(selectionText()).then(done);
    });
  }

  // Visionneuse.
  var box = document.querySelector(".lightbox");
  var image = box.querySelector(".stage img");
  var label = box.querySelector(".label");
  var boxPick = box.querySelector(".pick");
  var download = box.querySelector(".download");
  var current = -1;
  var opener = null;

  function syncLightbox() {
    if (current < 0) return;
    var n = Number(tiles[current].dataset.n);
    if (boxPick) {
      boxPick.setAttribute("aria-pressed", String(picked.has(n)));
      boxPick.setAttribute("aria-label", fill(picked.has(n) ? t.unpick : t.pick, { n: n }));
    }
  }

  function show(index) {
    current = (index + tiles.length) % tiles.length;
    var tile = tiles[current];
    var n = Number(tile.dataset.n);
    image.src = tile.dataset.full;
    image.alt = photoLabel(n);
    label.textContent = photoLabel(n) + " · " + (current + 1) + " / " + tiles.length;
    if (download) {
      download.href = tile.dataset.full;
      download.setAttribute("download", tile.dataset.file);
    }
    syncLightbox();
  }

  function open(index) {
    opener = document.activeElement;
    box.hidden = false;
    document.body.style.overflow = "hidden";
    show(index);
    box.querySelector(".close").focus();
  }

  function close() {
    box.hidden = true;
    document.body.style.overflow = "";
    current = -1;
    if (opener) opener.focus();
  }

  box.querySelector(".close").addEventListener("click", close);
  box.querySelector(".prev").addEventListener("click", function () {
    show(current - 1);
  });
  box.querySelector(".next").addEventListener("click", function () {
    show(current + 1);
  });
  if (boxPick) {
    boxPick.addEventListener("click", function () {
      toggle(Number(tiles[current].dataset.n));
    });
  }
  document.addEventListener("keydown", function (e) {
    if (box.hidden) return;
    if (e.key === "Escape") close();
    else if (e.key === "ArrowLeft") show(current - 1);
    else if (e.key === "ArrowRight") show(current + 1);
  });

  refresh();
})();
