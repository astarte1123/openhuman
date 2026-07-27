import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { ApiKeyCard, EnvField, ThinkingPhrasesField } from './Settings';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock('../api', () => ({
  envIsSet:           vi.fn(),
  envGet:             vi.fn(),
  setEnvVar:          vi.fn(),
  configGet:          vi.fn(),
  settingsGet:        vi.fn(),
  settingsSet:        vi.fn(),
  listSkills:         vi.fn(),
  toggleSkill:        vi.fn(),
  resetAllData:       vi.fn(),
  embeddingConfigGet: vi.fn(),
  embeddingConfigSet: vi.fn(),
  embeddingTest:      vi.fn(),
  licenseStatus:      vi.fn(),
  licenseActivate:    vi.fn(),
  licenseStartTrial:  vi.fn(),
  licenseDeactivate:  vi.fn(),
  licenseServerUrlSet:vi.fn(),
  licenseMachineId:   vi.fn(),
}));

import * as api from '../api';

const SAMPLE_FIELD = {
  envKey:      'ANTHROPIC_API_KEY',
  label:       'Anthropic API Key',
  placeholder: 'sk-ant-…',
  helpText:    'Priority 1',
};

// ---------------------------------------------------------------------------
// ApiKeyCard — input type="password" has no ARIA role; use aria-label query
// ---------------------------------------------------------------------------

describe('ApiKeyCard', () => {
  beforeEach(() => {
    vi.mocked(api.envIsSet).mockResolvedValue(false);
    vi.mocked(api.setEnvVar).mockResolvedValue(undefined);
  });

  afterEach(() => vi.clearAllMocks());

  it('renders the field label', () => {
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    expect(screen.getByText('Anthropic API Key')).toBeInTheDocument();
  });

  it('renders help text', () => {
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    expect(screen.getByText('Priority 1')).toBeInTheDocument();
  });

  it('renders password input hidden by default', () => {
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    const input = screen.getByLabelText('Anthropic API Key') as HTMLInputElement;
    expect(input.type).toBe('password');
  });

  it('shows Save button', async () => {
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalledWith('ANTHROPIC_API_KEY'));
    // Save button exists (may be disabled while empty, that is correct)
    expect(screen.getByRole('button', { name: /settings\.save|Save/i })).toBeInTheDocument();
  });

  it('shows key_set badge and Remove button when key is already set', async () => {
    vi.mocked(api.envIsSet).mockResolvedValue(true);
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalledWith('ANTHROPIC_API_KEY'));
    expect(screen.getByText(/settings\.key_set|Key saved/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /settings\.key_remove|Remove/i })).toBeInTheDocument();
  });

  it('calls setEnvVar with key and value on save', async () => {
    const user = userEvent.setup();
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalled());

    const input = screen.getByLabelText('Anthropic API Key');
    await user.type(input, 'sk-ant-test-value');
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('ANTHROPIC_API_KEY', 'sk-ant-test-value');
    });
  });

  it('saves on Enter key', async () => {
    const user = userEvent.setup();
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalled());

    const input = screen.getByLabelText('Anthropic API Key');
    await user.type(input, 'sk-enter-key{Enter}');

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('ANTHROPIC_API_KEY', 'sk-enter-key');
    });
  });

  it('calls setEnvVar with empty string on Remove', async () => {
    vi.mocked(api.envIsSet).mockResolvedValue(true);
    const user = userEvent.setup();
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /settings\.key_remove|Remove/i }));

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('ANTHROPIC_API_KEY', '');
    });
  });

  it('calls onSaved callback after successful save', async () => {
    const onSaved = vi.fn();
    const user = userEvent.setup();
    render(<ApiKeyCard field={SAMPLE_FIELD} onSaved={onSaved} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalled());

    const input = screen.getByLabelText('Anthropic API Key');
    await user.type(input, 'sk-callback-test');
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
  });

  it('toggles input to text type when eye button clicked', async () => {
    const user = userEvent.setup();
    render(<ApiKeyCard field={SAMPLE_FIELD} />);

    const input = screen.getByLabelText('Anthropic API Key') as HTMLInputElement;
    await user.type(input, 'sk-visible');

    const eyeBtn = screen.getByRole('button', { name: /settings\.show_key|Show key/i });
    await user.click(eyeBtn);

    expect((screen.getByLabelText('Anthropic API Key') as HTMLInputElement).type).toBe('text');
  });

  it('shows active badge when isActive prop is true', async () => {
    vi.mocked(api.envIsSet).mockResolvedValue(true);
    render(<ApiKeyCard field={SAMPLE_FIELD} isActive />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalled());
    expect(screen.getByText(/settings\.key_active|Active/)).toBeInTheDocument();
  });

  it('shows saved badge after successful save', async () => {
    const user = userEvent.setup();
    render(<ApiKeyCard field={SAMPLE_FIELD} />);
    await waitFor(() => expect(api.envIsSet).toHaveBeenCalled());

    const input = screen.getByLabelText('Anthropic API Key');
    await user.type(input, 'sk-saved-badge');
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => expect(screen.getByText(/settings\.saved|Saved/)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// EnvField — input type="text", role=textbox
// ---------------------------------------------------------------------------

describe('EnvField', () => {
  const ENV_PROPS = {
    envKey:      'CUSTOM_AI_URL',
    label:       'Base URL',
    placeholder: 'http://localhost:1234',
    helpText:    'Custom AI endpoint',
  };

  beforeEach(() => {
    vi.mocked(api.envGet).mockResolvedValue('');
    vi.mocked(api.setEnvVar).mockResolvedValue(undefined);
    vi.mocked(api.settingsSet).mockResolvedValue(undefined);
  });

  afterEach(() => vi.clearAllMocks());

  it('renders label', () => {
    render(<EnvField {...ENV_PROPS} />);
    expect(screen.getByText('Base URL')).toBeInTheDocument();
  });

  it('renders help text', () => {
    render(<EnvField {...ENV_PROPS} />);
    expect(screen.getByText('Custom AI endpoint')).toBeInTheDocument();
  });

  it('loads value from envGet on mount', async () => {
    vi.mocked(api.envGet).mockResolvedValue('http://10.20.10.24:8080/v1/');
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalledWith('CUSTOM_AI_URL'));
    const input = screen.getByLabelText('Base URL') as HTMLInputElement;
    expect(input.value).toBe('http://10.20.10.24:8080/v1/');
  });

  it('Clear button not visible when value is empty', async () => {
    vi.mocked(api.envGet).mockResolvedValue('');
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());
    expect(screen.queryByRole('button', { name: /settings\.clear|Clear/i })).not.toBeInTheDocument();
  });

  it('Clear button visible when value non-empty', async () => {
    vi.mocked(api.envGet).mockResolvedValue('http://localhost:1234');
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: /settings\.clear|Clear/i })).toBeInTheDocument();
  });

  it('calls setEnvVar with value on save', async () => {
    const user = userEvent.setup();
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    const input = screen.getByLabelText('Base URL');
    await user.clear(input);
    await user.type(input, 'http://newserver:8080');
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('CUSTOM_AI_URL', 'http://newserver:8080');
    });
  });

  it('calls setEnvVar with empty string on Clear', async () => {
    vi.mocked(api.envGet).mockResolvedValue('http://localhost:1234');
    const user = userEvent.setup();
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /settings\.clear|Clear/i }));

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('CUSTOM_AI_URL', '');
    });
  });

  it('calls onSaved after successful save', async () => {
    const onSaved = vi.fn();
    const user = userEvent.setup();
    render(<EnvField {...ENV_PROPS} onSaved={onSaved} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    const input = screen.getByLabelText('Base URL');
    await user.type(input, 'http://test:9000');
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => expect(onSaved).toHaveBeenCalled());
  });

  it('saves on Enter key', async () => {
    const user = userEvent.setup();
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    const input = screen.getByLabelText('Base URL');
    await user.type(input, 'http://enter-test:1234{Enter}');

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('CUSTOM_AI_URL', 'http://enter-test:1234');
    });
  });

  it('shows saved badge after save', async () => {
    const user = userEvent.setup();
    render(<EnvField {...ENV_PROPS} />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    const input = screen.getByLabelText('Base URL');
    await user.type(input, 'http://save-badge:8080');
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => expect(screen.getByText(/settings\.saved|Saved/)).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// ThinkingPhrasesField
// ---------------------------------------------------------------------------

describe('ThinkingPhrasesField', () => {
  beforeEach(() => {
    vi.mocked(api.envGet).mockResolvedValue('');
    vi.mocked(api.setEnvVar).mockResolvedValue(undefined);
  });

  afterEach(() => vi.clearAllMocks());

  it('renders Default and Custom tab buttons', () => {
    render(<ThinkingPhrasesField />);
    expect(screen.getByRole('button', { name: /settings\.thinking_tab_default|Default/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /settings\.thinking_tab_custom|Custom/i })).toBeInTheDocument();
  });

  it('Default tab is active on first render', () => {
    render(<ThinkingPhrasesField />);
    const defaultBtn = screen.getByRole('button', { name: /settings\.thinking_tab_default|Default/i });
    expect(defaultBtn.className).toContain('border-primary-500');
  });

  it('Default tab shows read-only textarea containing default phrases', async () => {
    render(<ThinkingPhrasesField />);
    const readOnly = document.querySelector('textarea[readonly]') as HTMLTextAreaElement;
    expect(readOnly).toBeInTheDocument();
    expect(readOnly.value).toContain('interfacing');
  });

  it('Default tab has a Copy button', () => {
    render(<ThinkingPhrasesField />);
    expect(screen.getByRole('button', { name: /settings\.copy|Copia|Copy/i })).toBeInTheDocument();
  });

  it('switching to Custom tab shows editable textarea', async () => {
    const user = userEvent.setup();
    render(<ThinkingPhrasesField />);

    await user.click(screen.getByRole('button', { name: /settings\.thinking_tab_custom|Custom/i }));

    const editable = document.querySelector('textarea:not([readonly])');
    expect(editable).toBeInTheDocument();
  });

  it('Custom tab Save calls setEnvVar with phrases', async () => {
    const user = userEvent.setup();
    render(<ThinkingPhrasesField />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalledWith('YOUMA_THINKING_PHRASES'));

    await user.click(screen.getByRole('button', { name: /settings\.thinking_tab_custom|Custom/i }));

    const textarea = document.querySelector('textarea:not([readonly])')!;
    await user.clear(textarea);
    await user.type(textarea, 'thinking, processing, loading');

    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith(
        'YOUMA_THINKING_PHRASES',
        expect.stringContaining('thinking'),
      );
    });
  });

  it('Custom tab Clear calls setEnvVar with empty string', async () => {
    vi.mocked(api.envGet).mockResolvedValue('custom phrase one, custom phrase two');
    const user = userEvent.setup();
    render(<ThinkingPhrasesField />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /settings\.thinking_tab_custom|Custom/i }));

    const clearBtn = screen.getByRole('button', { name: /settings\.clear|Clear/i });
    await user.click(clearBtn);

    await waitFor(() => {
      expect(api.setEnvVar).toHaveBeenCalledWith('YOUMA_THINKING_PHRASES', '');
    });
  });

  it('loads saved custom phrases into Custom tab textarea', async () => {
    const saved = 'alpha, beta, gamma';
    vi.mocked(api.envGet).mockResolvedValue(saved);
    render(<ThinkingPhrasesField />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalledWith('YOUMA_THINKING_PHRASES'));

    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /settings\.thinking_tab_custom|Custom/i }));

    const textarea = document.querySelector('textarea:not([readonly])')! as HTMLTextAreaElement;
    expect(textarea.value).toBe(saved);
  });

  it('shows saved confirmation badge after successful save', async () => {
    const user = userEvent.setup();
    render(<ThinkingPhrasesField />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /settings\.thinking_tab_custom|Custom/i }));
    await user.click(screen.getByRole('button', { name: /settings\.save|Save/i }));

    await waitFor(() => expect(screen.getByText(/settings\.saved|Saved/)).toBeInTheDocument());
  });

  it('shows thinking_default_overridden warning when custom phrases differ from Milanese default', async () => {
    vi.mocked(api.envGet).mockResolvedValue('my custom phrase');
    render(<ThinkingPhrasesField />);
    await waitFor(() => expect(api.envGet).toHaveBeenCalled());

    // Warning appears on Default tab when custom phrases are different from Milanese
    expect(
      screen.getByText(/settings\.thinking_default_overridden|Not currently active/)
    ).toBeInTheDocument();
  });
});
