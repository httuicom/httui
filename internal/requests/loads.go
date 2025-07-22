package requests

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/tree/v2"
)

func (m *Model) LoadDefault() tea.Msg {
	config, _ := m.DefaultsRepo.First()
	return *config
}

func (m *Model) LoadWorspace() tea.Msg {
	config, _ := m.DefaultsRepo.First()
	workspace, _ := m.WorkspacesRepo.FindOne(config.WorkspaceId)

	return workspace
}

type RequestsData struct {
	List        []storage.Request
	Current     storage.Request
	RequestTree []tree.Node[storage.Request]
	ParentID    *uint
	Cursor      int
	Page        int
}

func (m *Model) LoadRequests() tea.Msg {
	config, _ := m.DefaultsRepo.First()
	request, _ := m.RequestsRepo.FindOne(config.RequestId)
	requests, _ := m.RequestsRepo.List(request.ParentID, "")

	cursor := 0
	page := 0

	if config.Cursor != nil {
		cursor = *config.Cursor
	}

	if config.Page != nil {
		page = *config.Page
	}

	return RequestsData{
		RequestTree: config.RequestTree.Data(),
		Cursor:      cursor,
		Page:        page,
		Current:     *request,
		List:        requests,
		ParentID:    request.ParentID,
	}
}

func (m *Model) LoadRequestsByParentId(parentId *uint) tea.Cmd {
	return func() tea.Msg {
		config, _ := m.DefaultsRepo.First()
		request, _ := m.RequestsRepo.FindOne(config.RequestId)
		requests, _ := m.RequestsRepo.List(parentId, "")

		return RequestsData{
			List:        requests,
			ParentID:    parentId,
			RequestTree: config.RequestTree.Data(),
			Cursor:      *config.Cursor,
			Page:        *config.Page,
			Current:     *request,
		}
	}
}

func (m *Model) LoadRequestsByFilter(filter string) tea.Cmd {
	return func() tea.Msg {
		config, _ := m.DefaultsRepo.First()
		request, _ := m.RequestsRepo.FindOne(config.RequestId)
		requests, _ := m.RequestsRepo.List(nil, filter)

		return RequestsData{
			RequestTree: config.RequestTree.Data(),
			List:        requests,
			Current:     *request,
			ParentID:    nil,
			Cursor:      0,
			Page:        0,
		}
	}
}
