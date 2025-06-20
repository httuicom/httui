package config

type Sync struct {
	Remote  bool `yaml:"remote"`
	Locally bool `yaml:"locally"`
}

type AutoSync struct {
	BeforeOpen   Sync `yaml:"beforeOpen"`
	BeforeCreate Sync `yaml:"beforeCreate"`
	BeforeUpdate Sync `yaml:"beforeUpdate"`
	BeforeDelete Sync `yaml:"beforeDelete"`
}

// SettingsConfig struct represents the config for the settings.
type SettingsConfig struct {
	Local    bool     `yaml:"local"`
	Token    string   `yaml:"token"`
	DeviceID string   `yaml:"deviceId"`
	AutoSync AutoSync `yaml:"autoSync"`
}

// config represents the main config for the application.
type config struct {
	Settings SettingsConfig `yaml:"settings"`
}

var Config = ConfigParser{}.GetDefaultConfig()
