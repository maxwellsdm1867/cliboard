// cliboard viewer — WebSocket updates with polling fallback, auto-scroll, selection, send-to-terminal

(function () {
    "use strict";

    // --- State ---
    var currentVersion = 0;
    var userScrolledUp = false;
    var previousBlockCount = 0;
    var pollTimer = null;
    var ws = null;
    var wsPort = null;
    var wsFailCount = 0;
    var wsMaxRetries = 3;
    var usingWebSocket = false;

    // --- Chat state ---
    var openChats = {};        // stepId -> true/false (which threads are open)
    var chatInputDrafts = {};  // stepId -> string (preserve drafts across re-renders)
    var chatMessages = [];     // all messages from server
    var selectionContext = null; // stored context from "Ask about this"

    // --- DOM refs ---
    var boardTitle = document.getElementById("board-title");
    var stepCount = document.getElementById("step-count");
    var boardContent = document.getElementById("board-content");
    var sendBtn = document.getElementById("send-btn");
    var toast = document.getElementById("toast");

    // --- WebSocket ---

    function connectWebSocket() {
        if (!wsPort) return;
        if (wsFailCount >= wsMaxRetries) {
            // Too many failures, fall back to polling permanently
            startPolling();
            return;
        }

        var wsUrl = "ws://" + window.location.hostname + ":" + wsPort;
        try {
            ws = new WebSocket(wsUrl);
        } catch (e) {
            wsFailCount++;
            startPolling();
            return;
        }

        ws.onopen = function () {
            wsFailCount = 0;
            usingWebSocket = true;
            // Stop polling since WebSocket is connected
            stopPolling();
        };

        ws.onmessage = function (event) {
            try {
                var data = JSON.parse(event.data);
                if (data.type === "chat_thinking") {
                    showThinkingIndicator(data.step_id);
                } else if (data.type === "chat_update") {
                    console.log("[ws] Received chat_update via WebSocket:", (data.messages || []).length, "messages");
                    var prevMessages = chatMessages;
                    chatMessages = data.messages || [];
                    // Only remove thinking indicator when a new assistant message arrives
                    // (not when the user's own message triggers a chat_update)
                    var hadAssistant = prevMessages.filter(function(m) { return m.role === "assistant"; }).length;
                    var hasAssistant = chatMessages.filter(function(m) { return m.role === "assistant"; }).length;
                    if (hasAssistant > hadAssistant) {
                        removeThinkingIndicators();
                    }
                    renderChatMessages();
                    stopChatPoll(); // Got update via WS, stop polling
                } else {
                    // Board update (data.type === "board_update" or no type for backwards compat)
                    if (data.version > currentVersion) {
                        update(data);
                        currentVersion = data.version;
                    }
                }
            } catch (e) {
                // Ignore malformed messages
            }
        };

        ws.onclose = function () {
            usingWebSocket = false;
            ws = null;
            wsFailCount++;
            if (wsFailCount < wsMaxRetries) {
                // Try to reconnect after a delay
                setTimeout(connectWebSocket, 1000);
            } else {
                // Fall back to polling
                startPolling();
            }
        };

        ws.onerror = function () {
            // onclose will fire after onerror, so let onclose handle reconnect
            if (ws) {
                ws.close();
            }
        };
    }

    // --- Polling (fallback) ---

    function startPolling() {
        if (!pollTimer) {
            poll();
        }
    }

    function stopPolling() {
        if (pollTimer) {
            clearTimeout(pollTimer);
            pollTimer = null;
        }
    }

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
            // data = { version, title, blocks_html, ws_port }

            // If we got a ws_port and haven't established WebSocket yet, try it
            if (data.ws_port && !usingWebSocket && wsFailCount < wsMaxRetries) {
                wsPort = data.ws_port;
                connectWebSocket();
                // Don't stop polling yet — WebSocket.onopen will stop it
            }

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
            // User is selecting — defer this update, it'll arrive on next push/poll
            return;
        }

        lastBlocksHtml = data.blocks_html;

        // Hide selection buttons before DOM replacement
        hideSendBtn();

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

        // Inject chat UI into steps
        injectChatUI();

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

    // Check if node is inside a reply, return { replyEquation, userQuestion } or null
    function getReplyContext(node) {
        var el = node.nodeType === Node.TEXT_NODE ? node.parentElement : node;
        var replyEq = null;
        var chatMsg = null;
        while (el && el !== document.body) {
            if (el.classList && el.classList.contains("reply-equation")) replyEq = el;
            if (el.classList && el.classList.contains("cb-chat-msg") && el.classList.contains("assistant")) chatMsg = el;
            el = el.parentElement;
        }
        if (!chatMsg) return null;
        // Find preceding user message in the same thread
        var userQuestion = "";
        var prev = chatMsg.previousElementSibling;
        while (prev) {
            if (prev.classList && prev.classList.contains("cb-chat-msg") && prev.classList.contains("user")) {
                // Get the text content, skip context/time divs
                var textDiv = prev.querySelector("div:not(.cb-chat-context):not(.cb-chat-time)");
                userQuestion = textDiv ? textDiv.textContent.trim() : prev.textContent.trim();
                break;
            }
            prev = prev.previousElementSibling;
        }
        return {
            latex: replyEq ? (replyEq.getAttribute("data-latex") || "") : "",
            eqNum: replyEq ? (replyEq.getAttribute("data-eq-num") || "") : "",
            userQuestion: userQuestion
        };
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

        var replyCtx = getReplyContext(range.startContainer);
        showSendBtn(range, stepEl, replyCtx);
    }

    // "Ask about this" button (created once, reused)
    var askBtn = document.createElement("button");
    askBtn.className = "cb-ask-btn hidden";
    askBtn.textContent = "? Ask about this";
    askBtn.style.position = "fixed";
    askBtn.style.zIndex = "100";
    askBtn.style.boxShadow = "0 4px 12px rgba(0, 0, 0, 0.4)";
    document.body.appendChild(askBtn);

    function showSendBtn(range, stepEl, replyCtx) {
        var rect = range.getBoundingClientRect();
        sendBtn.classList.remove("hidden");
        // Only show "Ask about this" for main step equations, not reply content
        if (replyCtx) {
            askBtn.classList.add("hidden");
        } else {
            askBtn.classList.remove("hidden");
        }

        // Measure buttons
        var sendWidth = sendBtn.offsetWidth;
        var askWidth = replyCtx ? 0 : askBtn.offsetWidth;
        var gap = replyCtx ? 0 : 6;
        var totalWidth = sendWidth + gap + askWidth;
        var left = rect.left + rect.width / 2 - totalWidth / 2;
        var top = rect.top - 40;

        // Keep within viewport
        if (left < 8) left = 8;
        if (left + totalWidth > window.innerWidth - 8) {
            left = window.innerWidth - totalWidth - 8;
        }
        if (top < 8) top = rect.bottom + 8;

        sendBtn.style.left = left + "px";
        sendBtn.style.top = top + "px";

        if (!replyCtx) {
            askBtn.style.left = (left + sendWidth + 6) + "px";
            askBtn.style.top = top + "px";
        }

        // Store context for click handlers
        sendBtn._stepEl = stepEl;
        sendBtn._replyCtx = replyCtx || null;
        askBtn._stepEl = stepEl;
    }

    function hideSendBtn() {
        sendBtn.classList.add("hidden");
        sendBtn._stepEl = null;
        sendBtn._replyCtx = null;
        askBtn.classList.add("hidden");
        askBtn._stepEl = null;
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
        var replyCtx = sendBtn._replyCtx;

        // For reply equations, use the reply equation's latex; otherwise use step's
        var latex = replyCtx && replyCtx.latex
            ? replyCtx.latex
            : (stepEl.getAttribute("data-latex") || "");

        // Get selected text as fallback
        var sel = window.getSelection();
        var selectedText = sel ? sel.toString().trim() : "";

        // Clear selection and hide button immediately
        if (sel) sel.removeAllRanges();
        hideSendBtn();

        // POST to /select — server does proper LaTeX→Unicode conversion
        var payload = {
            step_id: parseInt(stepId, 10) || 0,
            title: stepTitle,
            latex: latex,
            text: selectedText
        };
        // Add reply context if selecting from a reply
        if (replyCtx) {
            if (replyCtx.userQuestion) payload.reply_context = replyCtx.userQuestion;
            if (replyCtx.eqNum) payload.eq_num = replyCtx.eqNum;
        }

        fetch("/select", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        }).then(function (res) {
            return res.json();
        }).then(function (data) {
            var clipText = data.formatted || ("[Step " + stepId + "] " + selectedText);
            // Paste into chat input so user can keep typing around it
            var chatInput = document.querySelector('.cb-chat-input[data-step="' + stepId + '"]');
            if (chatInput) {
                // Open the chat thread
                openChats[stepId] = true;
                var thread = document.querySelector('.cb-chat-thread[data-step="' + stepId + '"]');
                if (thread) thread.classList.add("open");
                // Insert at cursor or append
                var before = chatInput.value.substring(0, chatInput.selectionStart || chatInput.value.length);
                var after = chatInput.value.substring(chatInput.selectionEnd || chatInput.value.length);
                chatInput.value = before + clipText + " " + after;
                chatInputDrafts[stepId] = chatInput.value;
                chatInput.focus();
                // Place cursor right after the inserted text
                var cursorPos = before.length + clipText.length + 1;
                chatInput.setSelectionRange(cursorPos, cursorPos);
            }
            showToast("\u2192 chat input");
        }).catch(function () {
            var clipText = "[Step " + stepId + "] " + selectedText;
            copyToClipboard(clipText).then(function () {
                showToast("Step " + stepId + " \u2192 clipboard");
            });
        });
    });

    // "Ask about this" button
    askBtn.addEventListener("mousedown", function (e) {
        e.preventDefault();
    });

    askBtn.addEventListener("click", function (e) {
        e.preventDefault();
        e.stopPropagation();

        var stepEl = askBtn._stepEl;
        if (!stepEl) return;

        var stepId = stepEl.getAttribute("data-step-id") || "1";
        var stepTitle = stepEl.getAttribute("data-step-title") || "";
        var latex = stepEl.getAttribute("data-latex") || "";

        var sel = window.getSelection();
        var selectedText = sel ? sel.toString().trim() : "";

        // Clear selection and hide buttons
        if (sel) sel.removeAllRanges();
        hideSendBtn();

        // Store selection context for when message is sent
        selectionContext = {
            selected: selectedText,
            latex: latex,
            step_title: stepTitle
        };

        // Open chat for this step
        openChats[stepId] = true;
        var thread = document.querySelector('.cb-chat-thread[data-step="' + stepId + '"]');
        if (thread) {
            thread.classList.add("open");
            var input = thread.querySelector(".cb-chat-input");
            if (input) {
                input.value = "What is " + selectedText + "?";
                chatInputDrafts[stepId] = input.value;
                input.focus();
            }
        }
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

    // --- Chat UI ---

    var chatToggleSvg = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>';

    function injectChatUI() {
        var steps = boardContent.querySelectorAll(".step");
        for (var i = 0; i < steps.length; i++) {
            var stepEl = steps[i];
            var stepId = stepEl.getAttribute("data-step-id");
            if (!stepId) continue;

            // Add chat toggle button to step header (if not already present)
            var header = stepEl.querySelector(".step-header");
            if (header && !header.querySelector(".cb-chat-toggle")) {
                var toggle = document.createElement("button");
                toggle.className = "cb-chat-toggle";
                toggle.setAttribute("data-step", stepId);
                toggle.title = "Chat about this step";
                toggle.innerHTML = chatToggleSvg + '<span class="cb-chat-badge" style="display:none">0</span>';
                header.appendChild(toggle);
            }

            // Add chat thread container inside step (if not already present)
            if (!stepEl.querySelector(".cb-chat-thread")) {
                var thread = document.createElement("div");
                thread.className = "cb-chat-thread";
                thread.setAttribute("data-step", stepId);
                thread.innerHTML =
                    '<div class="cb-chat-messages"></div>' +
                    '<div class="cb-chat-input-row">' +
                    '<input class="cb-chat-input" data-step="' + stepId + '" placeholder="Ask about this step..." />' +
                    '<button class="cb-chat-send" data-step="' + stepId + '">Send</button>' +
                    '</div>';
                stepEl.appendChild(thread);
            }
        }
        renderChatMessages();
    }

    function renderChatMessages() {
        var threads = document.querySelectorAll(".cb-chat-thread");
        for (var t = 0; t < threads.length; t++) {
            var thread = threads[t];
            var stepId = parseInt(thread.getAttribute("data-step"), 10);
            var msgs = chatMessages.filter(function (m) { return m.step_id === stepId; });
            var container = thread.querySelector(".cb-chat-messages");

            // Update badge
            var toggle = document.querySelector('.cb-chat-toggle[data-step="' + stepId + '"]');
            var badge = toggle ? toggle.querySelector(".cb-chat-badge") : null;
            if (badge) {
                if (msgs.length > 0) {
                    badge.textContent = msgs.length;
                    badge.style.display = "flex";
                } else {
                    badge.style.display = "none";
                }
            }

            container.innerHTML = msgs.map(function (m) {
                var contextHtml = m.context && m.context.selected
                    ? '<div class="cb-chat-context">Re: "' + escapeHtml(m.context.selected) + '"' +
                      (m.context.step_title ? " in " + escapeHtml(m.context.step_title) : "") + '</div>'
                    : "";
                // Assistant messages use server-rendered HTML (trusted); user messages are escaped
                var bodyHtml = m.role === "assistant" && m.rendered
                    ? m.rendered
                    : '<div>' + escapeHtml(m.text) + '</div>';
                return '<div class="cb-chat-msg ' + escapeHtml(m.role) + '">' +
                    contextHtml +
                    bodyHtml +
                    '<div class="cb-chat-time">' + new Date(m.timestamp).toLocaleTimeString() + '</div>' +
                    '</div>';
            }).join("");

            // Scroll to bottom of messages
            if (msgs.length > 0) container.scrollTop = container.scrollHeight;
        }

        // Restore open states
        Object.keys(openChats).forEach(function (stepId) {
            if (openChats[stepId]) {
                var thread = document.querySelector('.cb-chat-thread[data-step="' + stepId + '"]');
                if (thread) thread.classList.add("open");
            }
        });

        // Restore input drafts
        Object.keys(chatInputDrafts).forEach(function (stepId) {
            var input = document.querySelector('.cb-chat-input[data-step="' + stepId + '"]');
            if (input) input.value = chatInputDrafts[stepId];
        });
    }

    function escapeHtml(str) {
        if (!str) return "";
        return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
    }

    var chatPollTimer = null;
    var chatPollCount = 0;

    async function sendChatMessage(stepId, text, context) {
        try {
            var body = { step_id: stepId, text: text };
            if (context) body.context = context;

            var resp = await fetch("/chat", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(body)
            });
            if (resp.ok) {
                showToast("Question sent for step " + stepId);
                // Poll for reply (hook may take a few seconds)
                startChatPoll();
            }
        } catch (err) {
            console.error("Chat send error:", err);
        }
    }

    function startChatPoll() {
        stopChatPoll();
        chatPollCount = 0;
        chatPollTimer = setInterval(function () {
            chatPollCount++;
            fetchChat();
            if (chatPollCount >= 15) stopChatPoll(); // stop after 30s
        }, 2000);
    }

    function stopChatPoll() {
        if (chatPollTimer) {
            clearInterval(chatPollTimer);
            chatPollTimer = null;
        }
    }

    async function fetchChat() {
        try {
            var resp = await fetch("/chat");
            if (resp.ok) {
                var data = await resp.json();
                var newCount = (data.messages || []).length;
                if (newCount !== chatMessages.length) {
                    console.log("[poll] Got", newCount, "messages via GET /chat (was", chatMessages.length, ")");
                }
                chatMessages = data.messages || [];
                renderChatMessages();
            }
        } catch (err) {
            // Chat endpoint may not exist yet, silently ignore
        }
    }

    // Chat toggle click
    document.addEventListener("click", function (e) {
        var toggle = e.target.closest(".cb-chat-toggle");
        if (toggle) {
            var stepId = toggle.getAttribute("data-step");
            openChats[stepId] = !openChats[stepId];
            var thread = document.querySelector('.cb-chat-thread[data-step="' + stepId + '"]');
            if (thread) {
                thread.classList.toggle("open");
                if (thread.classList.contains("open")) {
                    var input = thread.querySelector(".cb-chat-input");
                    if (input) input.focus();
                }
            }
        }
    });

    // Send button click
    document.addEventListener("click", function (e) {
        if (e.target.classList.contains("cb-chat-send")) {
            var stepId = parseInt(e.target.getAttribute("data-step"), 10);
            var input = document.querySelector('.cb-chat-input[data-step="' + stepId + '"]');
            if (input && input.value.trim()) {
                var context = selectionContext;
                selectionContext = null;
                sendChatMessage(stepId, input.value.trim(), context);
                input.value = "";
                delete chatInputDrafts[stepId];
            }
        }
    });

    // Enter key in chat input
    document.addEventListener("keydown", function (e) {
        if (e.target.classList.contains("cb-chat-input") && e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            var stepId = parseInt(e.target.getAttribute("data-step"), 10);
            if (e.target.value.trim()) {
                var context = selectionContext;
                selectionContext = null;
                sendChatMessage(stepId, e.target.value.trim(), context);
                e.target.value = "";
                delete chatInputDrafts[stepId];
            }
        }
    });

    // Save drafts on input
    document.addEventListener("input", function (e) {
        if (e.target.classList.contains("cb-chat-input")) {
            chatInputDrafts[e.target.getAttribute("data-step")] = e.target.value;
        }
    });

    // --- Thinking indicator ---

    function showThinkingIndicator(stepId) {
        // Find the chat messages container for this step
        var step = document.querySelector('.step[data-step-id="' + stepId + '"]');
        if (!step) return;
        var messagesContainer = step.querySelector(".cb-chat-messages");
        if (!messagesContainer) return;

        // Don't add duplicate
        if (messagesContainer.querySelector(".cb-thinking")) return;

        var indicator = document.createElement("div");
        indicator.className = "cb-thinking";
        indicator.innerHTML = '<span class="cb-thinking-text">Thinking</span><span class="cb-thinking-dots"></span>';
        messagesContainer.appendChild(indicator);

        // Auto-scroll to show the indicator
        messagesContainer.scrollTop = messagesContainer.scrollHeight;

        // Open the chat thread if it's closed
        var thread = step.querySelector(".cb-chat-thread");
        if (thread && !thread.classList.contains("open")) {
            thread.classList.add("open");
        }
    }

    function removeThinkingIndicators() {
        var indicators = document.querySelectorAll(".cb-thinking");
        for (var i = 0; i < indicators.length; i++) {
            indicators[i].remove();
        }
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

    // --- Theme toggle ---

    function setTheme(theme) {
        document.documentElement.setAttribute("data-theme", theme);
        try { localStorage.setItem("cliboard-theme", theme); } catch (e) {}
        // Update active state on buttons
        var buttons = document.querySelectorAll("#theme-toggle button");
        for (var i = 0; i < buttons.length; i++) {
            if (buttons[i].getAttribute("data-theme") === theme) {
                buttons[i].classList.add("active");
            } else {
                buttons[i].classList.remove("active");
            }
        }
    }

    function initTheme() {
        var saved = null;
        try { saved = localStorage.getItem("cliboard-theme"); } catch (e) {}
        setTheme(saved || "dark");

        var buttons = document.querySelectorAll("#theme-toggle button");
        for (var i = 0; i < buttons.length; i++) {
            buttons[i].addEventListener("click", function () {
                setTheme(this.getAttribute("data-theme"));
            });
        }
    }

    // --- Init ---

    document.addEventListener("DOMContentLoaded", function () {
        // Show toast as hidden but ready
        toast.classList.remove("hidden");
        toast.style.opacity = "0";
        // Init theme
        initTheme();
        // Start with polling — it will discover ws_port and upgrade to WebSocket
        poll();
        // Fetch initial chat messages after first board load
        fetchChat();
    });
})();
