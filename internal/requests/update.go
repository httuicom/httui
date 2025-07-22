package requests

import (
	"fmt"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/common"
	"github.com/gandarfh/httui/pkg/convert"
	"github.com/gandarfh/httui/pkg/terminal"
	"github.com/gandarfh/httui/pkg/tree/v2"
	"gorm.io/gorm"
)

type UpdateRequestDefault storage.Request

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var (
		cmd  tea.Cmd
		cmds []tea.Cmd
	)

	switch msg := msg.(type) {
	case common.Sync:
		switch msg.Action {
		case "request":
			request := storage.Request{}
			if err := convert.ToSource(msg.Data, &request); err != nil {
				return m, nil
			}

			sync := true
			request.Sync = &sync

			if m.RequestsRepo.Sql.Model(&request).Session(&gorm.Session{FullSaveAssociations: true}).Where("external_id = ?", request.ExternalId).Updates(&request).RowsAffected == 0 {
				m.RequestsRepo.Sql.Model(&request).Create(&request)
			}

			return m, m.LoadRequestsByParentId(m.parentId)

		case "workspace":
			workspace := storage.Workspace{}
			if err := convert.ToSource(msg.Data, &workspace); err != nil {
				return m, nil
			}

			sync := true
			workspace.Sync = &sync

			if m.WorkspacesRepo.Sql.Model(&workspace).Session(&gorm.Session{FullSaveAssociations: true}).Where("external_id = ?", workspace.ExternalId).Updates(&workspace).RowsAffected == 0 {
				m.WorkspacesRepo.Sql.Model(&workspace).Create(&workspace)
			}
		}

	case RequestsData:
		m.Requests = msg
		m.parentId = msg.ParentID

		m.List.SetCursorAndPage(msg.Cursor, msg.Page)

		nodes := buildTree(m.Requests.List, nil)
		nodes = tree.MergeNodes(nodes, msg.RequestTree)
		nodes = tree.MergeNodes(nodes, m.List.Nodes())

		m.List.SetNodes(nodes)

		if len(m.Requests.List) > 0 {
			cmds = append(cmds, m.Detail.SetRequest(m.Requests.Current))
			cmds = append(cmds, func() tea.Msg { return UpdateRequestDefault(m.Requests.Current) })
		}

	case storage.Workspace:
		m.Workspace = msg
		m.List.Title = fmt.Sprintf("[%s]", msg.Name)

	case storage.Default:
		m.Configs = msg

	case UpdateRequestDefault:
		if m.Requests.Current.ID == msg.ID {
			m.DefaultsRepo.Update(storage.Default{
				RequestId: m.Requests.Current.ID,
			})
		}

	case Result:
		if msg.Err != nil {
			term := terminal.NewPreview(&msg.Err)
			return m, tea.Batch(common.SetLoading(false), term.OpenVim("Exec"))
		}

		term := terminal.NewPreview(&msg.Response)
		return m, tea.Batch(common.SetLoading(false), term.OpenVim("Exec"))

	case common.State:
		m, cmd = m.StateActions(msg)
		cmds = append(cmds, cmd)

	case tea.KeyMsg:
		if !m.command_active {
			m, cmd = m.KeyActions(msg)
			cmds = append(cmds, cmd)
		}

	case common.Environment:
		if msg.Reset {
			m.List.Title = fmt.Sprintf("[%s]", m.Workspace.Name)
			return m, nil
		} else {
			m.Workspace = msg.Workspace
			m.List.Title = fmt.Sprintf("[%s]", msg.Workspace.Name)
		}

		cmd = m.Detail.SetWorkspace(storage.Workspace(m.Workspace))
		cmds = append(cmds, cmd)

	case spinner.TickMsg:
		if m.loading.Value {
			m.spinner, cmd = m.spinner.Update(msg)
			m.List.Title = fmt.Sprintf("[%s]", m.Workspace.Name) + m.spinner.View() + m.loading.Msg
			cmds = append(cmds, cmd)
		}

	case common.Loading:
		m.loading = msg

		if m.loading.Value {
			cmds = append(cmds, m.spinner.Tick)
		}

	case terminal.Finish:
		m.List.Title = fmt.Sprintf("[%s]", m.Workspace.Name)
		m, cmd = m.TerminalActions(msg)

		cmds = append(cmds, cmd)

	case common.Command:
		m.command_active = msg.Active

	case common.CommandClose:
		m.command_active = false
		m, cmd = m.CommandsActions(msg)
		cmds = append(cmds, cmd)

	case tea.WindowSizeMsg:
		m.Height = msg.Height
		m.Width = msg.Width

		m.List.SetHeight(m.Height / 2)
		m.List.SetWidth(m.Width / 5)

		m.Detail.Width = m.Width - m.List.Width() - 2
		m.Detail.Height = m.Height - 9

		m.Detail.InputPreview.CharLimit = m.Detail.Width - m.Detail.Width/6
		m.Detail.InputName.CharLimit = m.Detail.Width / 2
	}

	if m.command_active {
		content, cmd := m.command_bar.Update(msg)
		m.command_bar = content.(common.Component)

		return m, cmd
	}

	m.List, cmd = m.List.Update(msg)
	cmds = append(cmds, cmd)

	m.Detail, cmd = m.Detail.Update(msg)
	cmds = append(cmds, cmd)

	return m, tea.Batch(cmds...)
}

func buildTree(requests []storage.Request, parentID *uint) []tree.Node[storage.Request] {
	var nodes []tree.Node[storage.Request]
	for _, req := range requests {
		if (req.ParentID == nil && parentID == nil) || (req.ParentID != nil && parentID != nil && *req.ParentID == *parentID) {

			children := buildTree(requests, &req.ID)
			node := tree.Node[storage.Request]{
				Value:    req.Name,
				Data:     req,
				Children: children,
				Expanded: false,
			}

			nodes = append(nodes, node)
		}
	}

	return nodes
}
