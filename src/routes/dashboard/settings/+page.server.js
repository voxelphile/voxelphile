/** @type {import('./$types').Actions} */
import {get_local_user_form_errors} from "../../../user-form.js";

export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();
        const errors = get_local_user_form_errors(formData);
        if (Object.keys(errors)) {
            return errors;
        }
        return { success: true };
	}
};