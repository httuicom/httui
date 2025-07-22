package details

import (
	"github.com/charmbracelet/bubbles/textinput"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/lipgloss"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/common"
)

type section uint

const (
	CursorTree    section = 0
	CursorName    section = 1
	CursorPreview section = 2
)

var (
	styleConfig   = glamour.DarkStyleConfig
	title_style   = lipgloss.NewStyle().Border(lipgloss.RoundedBorder())
	preview_style = lipgloss.NewStyle().Border(lipgloss.RoundedBorder())
)

type Model struct {
	Width          int
	Height         int
	Cursor         section
	InputName      textinput.Model
	InputPreview   textinput.Model
	bodyVP         viewport.Model
	headerVP       viewport.Model
	paramsVP       viewport.Model
	Request        storage.Request
	Workspace      storage.Workspace
	WorkspacesRepo *storage.WorkspacesRepo
	RequestsRepo   *storage.RequestsRepo
	ResponsesRepo  *storage.ResponsesRepo
	DefaultsRepo   *storage.DefaultsRepo
	EnvsRepo       *storage.EnvsRepo
}

func New(
	workspacesRepo *storage.WorkspacesRepo,
	requestsRepo *storage.RequestsRepo,
	responsesRepo *storage.ResponsesRepo,
	defaultsRepo *storage.DefaultsRepo,
	envsRepo *storage.EnvsRepo,
) Model {
	tn := textinput.New()
	tn.Prompt = ""
	tn.CharLimit = 156
	tn.Blur()

	tp := textinput.New()
	tp.Prompt = ""
	tp.CharLimit = 156
	tp.Blur()

	return Model{
		Cursor:         CursorTree,
		InputName:      tn,
		InputPreview:   tp,
		WorkspacesRepo: workspacesRepo,
		RequestsRepo:   requestsRepo,
		ResponsesRepo:  responsesRepo,
		DefaultsRepo:   defaultsRepo,
		EnvsRepo:       envsRepo,
	}
}

func (m *Model) SetWorkspace(w storage.Workspace) tea.Cmd {
	return func() tea.Msg {
		return w
	}
}

func (m *Model) SetRequest(r storage.Request) tea.Cmd {
	return func() tea.Msg {
		return r
	}
}

func (m *Model) Next() tea.Cmd {
	switch m.Cursor {
	case CursorName:
		m.InputName.Blur()
	case CursorPreview:
		m.InputPreview.Blur()
	}

	m.Cursor++

	if m.Cursor > 2 {
		m.Cursor = 0
	}

	m.CompleteInputValue()

	switch m.Cursor {
	case CursorName:
		return m.InputName.Focus()
	case CursorPreview:
		return m.InputPreview.Focus()
	}

	return nil
}

func (m *Model) Prev() tea.Cmd {
	switch m.Cursor {
	case CursorName:
		m.InputName.Blur()
	case CursorPreview:
		m.InputPreview.Blur()
	}

	m.Cursor--

	if m.Cursor < 0 {
		m.Cursor = CursorPreview
	}

	m.CompleteInputValue()

	switch m.Cursor {
	case CursorName:
		return m.InputName.Focus()
	case CursorPreview:
		return m.InputPreview.Focus()
	}

	return nil
}

func (m *Model) CompleteInputValue() {
	switch m.Cursor {
	case CursorName:
		m.InputName.SetValue(m.Request.Name)
	case CursorPreview:
		m.InputPreview.SetValue(m.Request.Endpoint)
	default:
		m.InputName.SetValue("")
		m.InputPreview.SetValue("")
	}
}

func (m Model) Init() tea.Cmd {
	return textinput.Blink
}

func (m Model) Update(msg tea.Msg) (Model, tea.Cmd) {
	var (
		cmd  tea.Cmd
		cmds []tea.Cmd
	)

	switch msg := msg.(type) {
	case common.State:
		if msg == common.Start_state {
			m.bodyVP = viewport.New(m.Width-m.Width/3-4, m.Height)
			m.headerVP = viewport.New(m.Width/3-2, m.Height/2-2)
			m.paramsVP = viewport.New(m.Width/3-2, m.Height/2-2)
		}

	case storage.Request:
		m.Request = msg

	case storage.Workspace:
		m.Workspace = msg
	}

	m.bodyVP, cmd = m.bodyVP.Update(msg)
	cmds = append(cmds, cmd)

	m.headerVP, cmd = m.headerVP.Update(msg)
	cmds = append(cmds, cmd)

	m.paramsVP, cmd = m.paramsVP.Update(msg)
	cmds = append(cmds, cmd)

	m.InputName, cmd = m.InputName.Update(msg)
	cmds = append(cmds, cmd)

	m.InputPreview, cmd = m.InputPreview.Update(msg)
	cmds = append(cmds, cmd)

	return m, tea.Batch(cmds...)
}

func (m Model) View() string {
	m.bodyVP.SetContent(m.Body())
	m.headerVP.SetContent(m.Headers())
	m.paramsVP.SetContent(m.Params())

	paramsContent := lipgloss.
		NewStyle().
		Height(m.Height).
		Width(m.Width / 3).
		Render(lipgloss.JoinVertical(
			lipgloss.Left,
			m.paramsVP.View(),
			lipgloss.NewStyle().Width(m.Width/3).Border(lipgloss.NormalBorder(), false).BorderBottom(true).String(),
			m.headerVP.View(),
		))

	content_row := lipgloss.NewStyle().Width(m.Width).Height(m.Height).Border(lipgloss.RoundedBorder()).Render(lipgloss.JoinHorizontal(
		lipgloss.Left,
		m.bodyVP.View(),
		lipgloss.NewStyle().Height(m.Height).Width(3).Border(lipgloss.NormalBorder(), false).BorderRight(true).String(),
		paramsContent,
	))

	container := lipgloss.NewStyle().Padding(0, 1).Render(lipgloss.JoinVertical(
		lipgloss.Top,
		m.Title(),
		m.Preview(),
		content_row,
	))

	return container
}
