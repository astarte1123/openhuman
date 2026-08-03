import type { User } from '../../types/api';
import { callCoreCommand } from '../coreCommandClient';

/**
 * User API endpoints
 */
export const userApi = {
  /**
   * Get current authenticated user information
   * Core RPC -> GET /auth/me
   */
  getMe: async (): Promise<User> => {
    return await callCoreCommand<User>('aura.auth_get_me');
  },
};
