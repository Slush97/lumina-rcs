// lumina-bridge speaks newline-delimited JSON over stdio.
// Tauri spawns this process and proxies UI calls into libgm.
package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/rs/zerolog"
	"go.mau.fi/mautrix-gmessages/pkg/libgm"
	"go.mau.fi/mautrix-gmessages/pkg/libgm/events"
	"go.mau.fi/mautrix-gmessages/pkg/libgm/gmproto"
)

type request struct {
	ID     string          `json:"id"`
	Method string          `json:"method"`
	Params json.RawMessage `json:"params,omitempty"`
}

type response struct {
	ID     string `json:"id"`
	Result any    `json:"result,omitempty"`
	Error  string `json:"error,omitempty"`
}

type eventOut struct {
	Event string `json:"event"`
	Data  any    `json:"data,omitempty"`
}

type server struct {
	log     zerolog.Logger
	dataDir string

	mu     sync.Mutex
	cli    *libgm.Client
	sess   *libgm.AuthData
	paired bool

	outMu sync.Mutex
	out   *json.Encoder
}

func newServer(log zerolog.Logger, dataDir string) *server {
	return &server{
		log:     log,
		dataDir: dataDir,
		out:     json.NewEncoder(os.Stdout),
	}
}

func (s *server) authPath() string { return filepath.Join(s.dataDir, "auth.json") }

func (s *server) loadSession() (*libgm.AuthData, error) {
	f, err := os.Open(s.authPath())
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var ad libgm.AuthData
	if err := json.NewDecoder(f).Decode(&ad); err != nil {
		return nil, err
	}
	return &ad, nil
}

func (s *server) saveSession() {
	s.mu.Lock()
	sess := s.sess
	s.mu.Unlock()
	if sess == nil {
		return
	}
	if err := os.MkdirAll(s.dataDir, 0o700); err != nil {
		s.log.Err(err).Msg("mkdir data dir")
		return
	}
	tmp := s.authPath() + ".tmp"
	f, err := os.OpenFile(tmp, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		s.log.Err(err).Msg("open auth file")
		return
	}
	if err := json.NewEncoder(f).Encode(sess); err != nil {
		f.Close()
		s.log.Err(err).Msg("encode auth")
		return
	}
	f.Close()
	if err := os.Rename(tmp, s.authPath()); err != nil {
		s.log.Err(err).Msg("rename auth file")
	}
}

func (s *server) emit(name string, data any) {
	s.outMu.Lock()
	defer s.outMu.Unlock()
	_ = s.out.Encode(eventOut{Event: name, Data: data})
}

func (s *server) reply(id string, result any) {
	s.outMu.Lock()
	defer s.outMu.Unlock()
	_ = s.out.Encode(response{ID: id, Result: result})
}

func (s *server) replyErr(id string, err error) {
	s.outMu.Lock()
	defer s.outMu.Unlock()
	_ = s.out.Encode(response{ID: id, Error: err.Error()})
}

func (s *server) handleEvent(rawEvt any) {
	switch evt := rawEvt.(type) {
	case *events.QR:
		s.emit("qr", map[string]string{"url": evt.URL})
	case *events.PairSuccessful:
		s.mu.Lock()
		s.paired = true
		cli := s.cli
		s.mu.Unlock()
		s.saveSession()
		s.emit("paired", map[string]string{"phone_id": evt.PhoneID})
		// libgm fires PairSuccessful, then internally Reconnects. There's
		// no ClientReady emitted from libgm core, so synthesize one when
		// the long-poll is actually up.
		go s.waitConnectedThenReady(cli)
	case *events.ClientReady:
		s.emit("ready", nil)
	case *events.AuthTokenRefreshed:
		s.saveSession()
	case *events.ListenFatalError:
		s.emit("error", map[string]string{"kind": "listen_fatal", "msg": evt.Error.Error()})
	case *events.ListenTemporaryError:
		s.emit("error", map[string]string{"kind": "listen_temporary", "msg": evt.Error.Error()})
	case *events.PhoneNotResponding:
		s.emit("phone_offline", nil)
	case *events.PhoneRespondingAgain:
		s.emit("phone_online", nil)
	case *gmproto.Conversation:
		s.emit("conversation_updated", convSummary(evt))
	default:
		s.log.Debug().Type("type", evt).Msg("unhandled libgm event")
	}
}

// waitConnectedThenReady polls IsConnected for up to 15s after a pair
// succeeds. libgm Reconnects internally but emits no signal when done,
// so this gives the UI a reliable handoff to switch to the conversation
// list screen.
func (s *server) waitConnectedThenReady(cli *libgm.Client) {
	if cli == nil {
		return
	}
	for i := 0; i < 30; i++ {
		time.Sleep(500 * time.Millisecond)
		if cli.IsConnected() {
			s.emit("ready", nil)
			return
		}
	}
	s.emit("error", map[string]string{
		"kind": "ready_timeout",
		"msg":  "long-poll did not connect within 15s after pairing",
	})
}

func convSummary(c *gmproto.Conversation) map[string]any {
	lm := c.GetLatestMessage()
	return map[string]any{
		"id":             c.GetConversationID(),
		"name":           c.GetName(),
		"snippet":        lm.GetDisplayContent(),
		"snippet_from":   lm.GetDisplayName(),
		"snippet_self":   lm.GetFromMe() != 0,
		"timestamp":      c.GetLastMessageTimestamp(),
		"unread":         c.GetUnread(),
		"is_group":       c.GetIsGroupChat(),
		"avatar_color":   c.GetAvatarHexColor(),
		"pinned":         c.GetPinned(),
		"read_only":      c.GetReadOnly(),
	}
}

// ---------- methods ----------

func (s *server) handleStatus() any {
	s.mu.Lock()
	defer s.mu.Unlock()
	connected := false
	if s.cli != nil {
		connected = s.cli.IsConnected()
	}
	hasSavedAuth := false
	if _, err := os.Stat(s.authPath()); err == nil {
		hasSavedAuth = true
	}
	return map[string]bool{
		"paired":    s.paired || hasSavedAuth,
		"connected": connected,
	}
}

func (s *server) handlePair() (any, error) {
	s.mu.Lock()
	if s.cli != nil && s.cli.IsConnected() {
		s.cli.Disconnect()
	}
	sess := libgm.NewAuthData()
	cli := libgm.NewClient(sess, nil, s.log)
	cli.SetEventHandler(s.handleEvent)
	s.sess = sess
	s.cli = cli
	s.paired = false
	s.mu.Unlock()

	qr, err := cli.StartLogin()
	if err != nil {
		return nil, fmt.Errorf("start login: %w", err)
	}
	s.emit("qr", map[string]string{"url": qr})
	return map[string]string{"qr_url": qr}, nil
}

func (s *server) handleConnect() (any, error) {
	sess, err := s.loadSession()
	if err != nil {
		return nil, fmt.Errorf("load saved session: %w", err)
	}
	s.mu.Lock()
	if s.cli != nil && s.cli.IsConnected() {
		s.cli.Disconnect()
	}
	cli := libgm.NewClient(sess, nil, s.log)
	cli.SetEventHandler(s.handleEvent)
	s.sess = sess
	s.cli = cli
	s.paired = sess.Mobile != nil
	s.mu.Unlock()

	if err := cli.Connect(); err != nil {
		return nil, fmt.Errorf("connect: %w", err)
	}
	return map[string]bool{"ok": true}, nil
}

func (s *server) handlePairGaia(params json.RawMessage) (any, error) {
	args := struct {
		Cookies map[string]string `json:"cookies"`
	}{}
	if err := json.Unmarshal(params, &args); err != nil {
		return nil, err
	}
	if _, ok := args.Cookies["SAPISID"]; !ok {
		return nil, errors.New("SAPISID cookie missing — login likely incomplete")
	}

	s.mu.Lock()
	if s.cli != nil && s.cli.IsConnected() {
		s.cli.Disconnect()
	}
	sess := libgm.NewAuthData()
	sess.SetCookies(args.Cookies)
	cli := libgm.NewClient(sess, nil, s.log)
	cli.SetEventHandler(s.handleEvent)
	s.sess = sess
	s.cli = cli
	s.paired = false
	s.mu.Unlock()

	go func() {
		err := cli.DoGaiaPairing(context.Background(), func(emoji string) {
			s.emit("gaia_emoji", map[string]string{"emoji": emoji})
		})
		if err != nil {
			s.emit("error", map[string]string{"kind": "gaia_pair", "msg": err.Error()})
		}
		// Persist whatever auth state we have, even on partial failure.
		s.saveSession()
	}()

	return map[string]bool{"started": true}, nil
}

func (s *server) handleUnpair() (any, error) {
	s.mu.Lock()
	cli := s.cli
	s.mu.Unlock()
	if cli != nil {
		_ = cli.Unpair()
		cli.Disconnect()
	}
	_ = os.Remove(s.authPath())
	s.mu.Lock()
	s.cli = nil
	s.sess = nil
	s.paired = false
	s.mu.Unlock()
	return map[string]bool{"ok": true}, nil
}

func (s *server) handleListConversations(params json.RawMessage) (any, error) {
	s.mu.Lock()
	cli := s.cli
	s.mu.Unlock()
	if cli == nil {
		return nil, errors.New("not connected")
	}
	args := struct {
		Count int `json:"count"`
	}{Count: 50}
	if len(params) > 0 {
		if err := json.Unmarshal(params, &args); err != nil {
			return nil, err
		}
	}
	resp, err := cli.ListConversations(args.Count, gmproto.ListConversationsRequest_INBOX)
	if err != nil {
		return nil, err
	}
	out := make([]map[string]any, 0, len(resp.GetConversations()))
	for _, c := range resp.GetConversations() {
		out = append(out, convSummary(c))
	}
	return out, nil
}

// ---------- dispatcher ----------

func (s *server) dispatch(req request) {
	defer func() {
		if r := recover(); r != nil {
			s.replyErr(req.ID, fmt.Errorf("panic: %v", r))
		}
	}()
	switch req.Method {
	case "status":
		s.reply(req.ID, s.handleStatus())
	case "pair":
		res, err := s.handlePair()
		if err != nil {
			s.replyErr(req.ID, err)
			return
		}
		s.reply(req.ID, res)
	case "pair_gaia":
		res, err := s.handlePairGaia(req.Params)
		if err != nil {
			s.replyErr(req.ID, err)
			return
		}
		s.reply(req.ID, res)
	case "connect":
		res, err := s.handleConnect()
		if err != nil {
			s.replyErr(req.ID, err)
			return
		}
		s.reply(req.ID, res)
	case "unpair":
		res, err := s.handleUnpair()
		if err != nil {
			s.replyErr(req.ID, err)
			return
		}
		s.reply(req.ID, res)
	case "list_conversations":
		res, err := s.handleListConversations(req.Params)
		if err != nil {
			s.replyErr(req.ID, err)
			return
		}
		s.reply(req.ID, res)
	default:
		s.replyErr(req.ID, fmt.Errorf("unknown method: %s", req.Method))
	}
}

func main() {
	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stderr, TimeFormat: time.RFC3339}).
		With().Timestamp().Logger()

	dataDir := os.Getenv("LUMINA_DATA_DIR")
	if dataDir == "" {
		home, _ := os.UserHomeDir()
		dataDir = filepath.Join(home, ".local", "share", "lumina-rcs")
	}
	log.Info().Str("data_dir", dataDir).Msg("lumina-bridge starting")

	s := newServer(log, dataDir)

	// Auto-resume if we have a saved session.
	if _, err := os.Stat(s.authPath()); err == nil {
		if _, err := s.handleConnect(); err != nil {
			log.Warn().Err(err).Msg("auto-connect failed; UI must call pair or connect")
		} else {
			log.Info().Msg("auto-connected from saved session")
		}
	}
	s.emit("hello", map[string]string{"version": "0.1.0"})

	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 0, 64*1024), 4*1024*1024)
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var req request
		if err := json.Unmarshal(line, &req); err != nil {
			log.Err(err).Bytes("line", line).Msg("bad request line")
			continue
		}
		go s.dispatch(req)
	}
	if err := scanner.Err(); err != nil {
		log.Err(err).Msg("stdin scanner error")
	}
	log.Info().Msg("stdin closed; exiting")
}
