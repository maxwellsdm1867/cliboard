// cliboard viewer — polling, auto-scroll, selection, send-to-terminal

(function () {
    "use strict";

    // --- State ---
    var currentVersion = 0;
    var userScrolledUp = false;
    var previousBlockCount = 0;
    var pollTimer = null;

    // --- DOM refs ---
    var boardTitle = document.getElementById("board-title");
    var stepCount = document.getElementById("step-count");
    var boardContent = document.getElementById("board-content");
    var sendBtn = document.getElementById("send-btn");
    var toast = document.getElementById("toast");

    // --- Polling ---

    async function poll() {
        try {
            var url = "/board";
            if (currentVersion > 0) {
                url += "?v=" + currentVersion;
            }
            var res = await fetch(url);
            if (res.status === 304) {
                // No changes, skip update
                schedulePoll();
                return;
            }
            if (!res.ok) {
                schedulePoll();
                return;
            }
            var data = await res.json();
            // data = { version, title, blocks_html }
            if (data.version > currentVersion) {
                update(data);
                currentVersion = data.version;
            }
        } catch (e) {
            // Server might be restarting, silently retry
        }
        schedulePoll();
    }

    function schedulePoll() {
        if (pollTimer) clearTimeout(pollTimer);
        pollTimer = setTimeout(poll, 500);
    }

    // --- Update DOM ---

    var lastBlocksHtml = "";

    function update(data) {
        // Update title
        if (data.title) {
            boardTitle.textContent = data.title;
            document.title = data.title + " - cliboard";
        }

        // Skip DOM update if HTML hasn't actually changed (preserves user selection)
        if (data.blocks_html === lastBlocksHtml) {
            return;
        }

        // Don't replace DOM while user has an active selection (would destroy it)
        var sel = window.getSelection();
        if (sel && !sel.isCollapsed && getStepAncestor(sel.anchorNode)) {
            // User is selecting — defer this update, it'll arrive on next poll
            return;
        }

        lastBlocksHtml = data.blocks_html;

        // Replace content
        boardContent.innerHTML = data.blocks_html;

        // Count steps and update badge
        var steps = boardContent.querySelectorAll(".step");
        var count = steps.length;
        if (count > 0) {
            stepCount.textContent = count + (count === 1 ? " step" : " steps");
        } else {
            stepCount.textContent = "";
        }

        // Mark new steps for animation
        if (count > previousBlockCount) {
            for (var i = previousBlockCount; i < count; i++) {
                steps[i].classList.add("new");
            }
        }
        previousBlockCount = count;

        // Auto-scroll if user hasn't scrolled up
        if (!userScrolledUp) {
            scrollToBottom();
        }
    }

    // --- Auto-scroll ---

    function scrollToBottom() {
        window.scrollTo({
            top: document.body.scrollHeight,
            behavior: "smooth"
        });
    }

    function isNearBottom() {
        var threshold = 100;
        var scrollPos = window.scrollY + window.innerHeight;
        var docHeight = document.body.scrollHeight;
        return docHeight - scrollPos < threshold;
    }

    var lastScrollY = 0;
    window.addEventListener("scroll", function () {
        var currentScrollY = window.scrollY;
        if (currentScrollY < lastScrollY && !isNearBottom()) {
            // User scrolled up and is not near bottom
            userScrolledUp = true;
        } else if (isNearBottom()) {
            // User scrolled back to bottom
            userScrolledUp = false;
        }
        lastScrollY = currentScrollY;
    }, { passive: true });

    // --- Selection + Send to terminal ---

    function getStepAncestor(node) {
        var el = node.nodeType === Node.TEXT_NODE ? node.parentElement : node;
        while (el && el !== document.body) {
            if (el.classList && el.classList.contains("step")) return el;
            el = el.parentElement;
        }
        return null;
    }

    function handleSelectionChange() {
        var sel = window.getSelection();
        if (!sel || sel.isCollapsed || !sel.rangeCount) {
            hideSendBtn();
            return;
        }

        var range = sel.getRangeAt(0);
        var stepEl = getStepAncestor(range.startContainer);
        if (!stepEl) {
            hideSendBtn();
            return;
        }

        showSendBtn(range, stepEl);
    }

    function showSendBtn(range, stepEl) {
        var rect = range.getBoundingClientRect();
        sendBtn.classList.remove("hidden");
        // Position above the selection, centered
        var btnWidth = sendBtn.offsetWidth;
        var left = rect.left + rect.width / 2 - btnWidth / 2;
        var top = rect.top - 40;

        // Keep within viewport
        if (left < 8) left = 8;
        if (left + btnWidth > window.innerWidth - 8) {
            left = window.innerWidth - btnWidth - 8;
        }
        if (top < 8) top = rect.bottom + 8;

        sendBtn.style.left = left + "px";
        sendBtn.style.top = top + "px";

        // Store the step element for the click handler
        sendBtn._stepEl = stepEl;
    }

    function hideSendBtn() {
        sendBtn.classList.add("hidden");
        sendBtn._stepEl = null;
    }

    document.addEventListener("mouseup", function () {
        // Small delay to let selection finalize
        setTimeout(handleSelectionChange, 10);
    });

    document.addEventListener("selectionchange", function () {
        var sel = window.getSelection();
        if (!sel || sel.isCollapsed) {
            hideSendBtn();
        }
    });

    // Send button click
    sendBtn.addEventListener("mousedown", function (e) {
        // Prevent the click from clearing the selection
        e.preventDefault();
    });

    sendBtn.addEventListener("click", function (e) {
        e.preventDefault();
        e.stopPropagation();

        var stepEl = sendBtn._stepEl;
        if (!stepEl) return;

        var stepId = stepEl.getAttribute("data-step-id") || "?";
        var stepTitle = stepEl.getAttribute("data-step-title") || "";
        var latex = stepEl.getAttribute("data-latex") || "";

        // Get selected text as fallback
        var sel = window.getSelection();
        var selectedText = sel ? sel.toString().trim() : "";

        // Clear selection and hide button immediately
        if (sel) sel.removeAllRanges();
        hideSendBtn();

        // POST to /select — server does proper LaTeX→Unicode conversion
        // Then copy the server's unicode result to clipboard
        fetch("/select", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                step_id: parseInt(stepId, 10) || 0,
                title: stepTitle,
                latex: latex,
                text: selectedText
            })
        }).then(function (res) {
            return res.json();
        }).then(function (data) {
            // Use server's properly converted unicode text
            var clipText = data.formatted || ("[Step " + stepId + "] " + selectedText);
            return copyToClipboard(clipText).then(function () {
                showToast("Step " + stepId + " \u2192 clipboard");
            });
        }).catch(function () {
            // Fallback: copy browser text if server fails
            var clipText = "[Step " + stepId + "] " + selectedText;
            copyToClipboard(clipText).then(function () {
                showToast("Step " + stepId + " \u2192 clipboard");
            });
        });
    });

    function copyToClipboard(text) {
        if (navigator.clipboard && navigator.clipboard.writeText) {
            return navigator.clipboard.writeText(text);
        }
        // Fallback for older browsers / non-HTTPS
        return new Promise(function (resolve, reject) {
            var ta = document.createElement("textarea");
            ta.value = text;
            ta.style.position = "fixed";
            ta.style.left = "-9999px";
            document.body.appendChild(ta);
            ta.select();
            try {
                document.execCommand("copy");
                resolve();
            } catch (err) {
                reject(err);
            }
            document.body.removeChild(ta);
        });
    }

    // --- Toast ---

    function showToast(msg) {
        toast.classList.remove("hidden");
        toast.textContent = msg;
        toast.style.opacity = "1";
        setTimeout(function () {
            toast.style.opacity = "0";
        }, 1500);
    }

    // --- Init ---

    document.addEventListener("DOMContentLoaded", function () {
        // Show toast as hidden but ready
        toast.classList.remove("hidden");
        toast.style.opacity = "0";
        // Start polling
        poll();
    });
})();
